//! Poses and skinning matrices (MODELS.md 2). A pose is a model-space rotation per bone about
//! its pivot, relative to the T-pose, plus an offset of the whole body. Because every bone's
//! rest frame is the model's own axes, the same pose drives any skeleton that has the standard
//! bones, whatever its proportions and whatever tool exported it.

use glam::{Mat4, Quat, Vec3};

use crate::rig::{BONES, NO_PARENT, PARENTS};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Translation of the whole body (bob, crouch, lying down).
    pub offset: Vec3,
    pub rot: [Quat; BONES],
}

impl Default for Pose {
    fn default() -> Self {
        Pose::REST
    }
}

impl Pose {
    /// The T-pose.
    pub const REST: Pose = Pose {
        offset: Vec3::ZERO,
        rot: [Quat::IDENTITY; BONES],
    };

    /// Shortest-path blend: `self` at 0, `other` at 1.
    pub fn blend(&self, other: &Pose, t: f32) -> Pose {
        let t = t.clamp(0.0, 1.0);
        let mut out = *self;
        out.offset = self.offset.lerp(other.offset, t);
        for (o, b) in out.rot.iter_mut().zip(&other.rot) {
            *o = o.lerp(*b, t);
        }
        out
    }
}

/// The grip (LOOK.md 6.3): how a prop sits in the right hand. A prop's business end (+X
/// in its own space) stands out of the fist across the forearm, forward and tipped
/// `GRIP_TILT_DEG` towards the elbow (the right arm points −Y in the T-pose and hangs
/// from there, so a hanging arm holds a blade level, its tip a little raised); its edge
/// (+Y) follows the knuckles, away from the elbow.
pub const GRIP_TILT_DEG: f32 = 12.0;

/// See [`GRIP_TILT_DEG`]: the prop's axes in the hand bone's space.
pub fn grip_right() -> Mat4 {
    let (sin, cos) = GRIP_TILT_DEG.to_radians().sin_cos();
    Mat4::from_cols(
        glam::Vec4::new(cos, sin, 0.0, 0.0),
        glam::Vec4::new(sin, -cos, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, -1.0, 0.0),
        glam::Vec4::W,
    )
}

/// Where the first-person view model sits (LOOK.md 6.4, CONTENT.md 3.1): a template's
/// `fit_view`, written like its `fit` in glTF's conventions (metres, degrees about X,
/// then Y, then Z, a scale; scaled, turned, moved), as a matrix in model space (+X
/// forward, +Z up, units) applied to the built prop before it is placed in the view. A
/// prop laid along the arm by its `fit` (a musket, turned 88° about Y) is turned back by
/// the same angle to point along the look.
pub fn view_fit(f: &[f32; 7]) -> Mat4 {
    const UNITS_PER_METRE: f32 = 32.0;
    let r = Mat4::from_rotation_z(f[5].to_radians())
        * Mat4::from_rotation_y(f[4].to_radians())
        * Mat4::from_rotation_x(f[3].to_radians());
    let gltf = Mat4::from_translation(Vec3::new(f[0], f[1], f[2]) * UNITS_PER_METRE)
        * r
        * Mat4::from_scale(Vec3::splat(f[6]));
    // glTF (+Y up, facing +Z) to model space (+Z up, facing +X): x, y, z -> z, x, y.
    let to_model = Mat4::from_cols(
        glam::Vec4::new(0.0, 1.0, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, 1.0, 0.0),
        glam::Vec4::new(1.0, 0.0, 0.0, 0.0),
        glam::Vec4::W,
    );
    to_model * gltf * to_model.transpose()
}

/// The grip of the left hand (LOOK.md 6.5): the right one mirrored through the body's
/// plane, so the business end stands forward and is tipped towards the elbow as it is on
/// the right and the edge (+Y) follows the knuckles away from the elbow; the prop's face
/// (+Z) then stands out of the back of the hand, where a shield's boss belongs. (A mirror
/// is not a rotation: this is the right grip turned half round about the business end, a
/// proper rotation that lays a blade's flat the other way, which no blade shows.)
pub fn grip_left() -> Mat4 {
    let (sin, cos) = GRIP_TILT_DEG.to_radians().sin_cos();
    Mat4::from_cols(
        glam::Vec4::new(cos, -sin, 0.0, 0.0),
        glam::Vec4::new(sin, cos, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, 1.0, 0.0),
        glam::Vec4::W,
    )
}

/// Where a held prop is drawn: the skinning matrix of `prop_r`, the translation to that
/// bone's pivot, the grip.
pub fn prop_attach(pivots: &[Vec3; BONES], skin: &[Mat4; BONES]) -> Mat4 {
    let at = crate::rig::bone::PROP_R;
    skin[at] * Mat4::from_translation(pivots[at]) * grip_right()
}

/// Where the off hand's prop is drawn (LOOK.md 6.5): as [`prop_attach`], on `prop_l`
/// with the left grip.
pub fn prop_attach_left(pivots: &[Vec3; BONES], skin: &[Mat4; BONES]) -> Mat4 {
    let at = crate::rig::bone::PROP_L;
    skin[at] * Mat4::from_translation(pivots[at]) * grip_left()
}

/// The sheath (LOOK.md 6.7, EQUIPMENT.md 5.3): how a prop rests on the back out of a
/// fight. The grip stands over the right shoulder, `SHEATH_LIFT` above, to the right and
/// out of `prop_back` (which is behind the chest, MODELS.md 2), and the business end (+X
/// of the prop) runs down across the back towards the left hip, `SHEATH_TILT_DEG` from
/// upright; the flat (±Z) lies against the back, so the edge (+Y) shows sideways. Model
/// units: up, to the right (−Y of model space, which faces +X with +Y to its left), and
/// out behind (−X). The hero's `prop_back` pivot sits 3 cm inside his shoulder blades
/// (the body's back is 16.5 cm behind the chest bone, the pivot 13.6), so the prop is
/// carried `SHEATH_OUT` behind it: the blade's inner face clears the back and the chest
/// overlays by a couple of centimetres.
pub const SHEATH_TILT_DEG: f32 = 32.0;
pub const SHEATH_OUT: f32 = 2.5;
pub const SHEATH_LIFT: Vec3 = Vec3::new(-SHEATH_OUT, -5.0, 10.0);
/// The off hand's sheath (LOOK.md 6.7): the mirror image, the grip over the left shoulder
/// and the business end down towards the right hip, one prop's thickness further out so
/// the two cross on the back with the off hand's over the right hand's.
pub const SHEATH_LIFT_LEFT: Vec3 = Vec3::new(-SHEATH_OUT - 1.0, 5.0, 10.0);
/// A shield's rest (LOOK.md 6.7): flat on the middle of the back at `prop_back`'s height
/// (a heater's 20 units of height reach from the shoulders to the hips), over the blades.
pub const SHEATH_LIFT_FLAT: Vec3 = Vec3::new(-SHEATH_OUT - 1.2, 0.0, 0.0);

/// See [`SHEATH_TILT_DEG`]: the prop's axes in the back bone's space.
pub fn grip_back() -> Mat4 {
    let (sin, cos) = SHEATH_TILT_DEG.to_radians().sin_cos();
    // +X (the business end) down and to the left; +Z (the flat) out of the back (−X of
    // the body); +Y = Z × X, a proper rotation.
    Mat4::from_cols(
        glam::Vec4::new(0.0, sin, -cos, 0.0),
        glam::Vec4::new(0.0, -cos, -sin, 0.0),
        glam::Vec4::new(-1.0, 0.0, 0.0, 0.0),
        glam::Vec4::W,
    )
}

/// Where a sheathed prop is drawn (LOOK.md 6.7): the skinning matrix of `prop_back`, the
/// translation to that bone's pivot lifted over the shoulder, the back grip.
pub fn prop_attach_back(pivots: &[Vec3; BONES], skin: &[Mat4; BONES]) -> Mat4 {
    let at = crate::rig::bone::PROP_BACK;
    skin[at] * Mat4::from_translation(pivots[at] + SHEATH_LIFT) * grip_back()
}

/// See [`SHEATH_LIFT_FLAT`]: a shield's prop space has its face along +Z (out, as it is
/// held) and its point along +X (down the hanging arm); on the back the face is out
/// (−X of the body) and the point down (−Z), a proper rotation.
pub fn grip_back_flat() -> Mat4 {
    Mat4::from_cols(
        glam::Vec4::new(0.0, 0.0, -1.0, 0.0),
        glam::Vec4::new(0.0, -1.0, 0.0, 0.0),
        glam::Vec4::new(-1.0, 0.0, 0.0, 0.0),
        glam::Vec4::W,
    )
}

/// Where a shield is drawn sheathed (LOOK.md 6.7): flat on the back at `prop_back`.
pub fn prop_attach_back_flat(pivots: &[Vec3; BONES], skin: &[Mat4; BONES]) -> Mat4 {
    let at = crate::rig::bone::PROP_BACK;
    skin[at] * Mat4::from_translation(pivots[at] + SHEATH_LIFT_FLAT) * grip_back_flat()
}

/// See [`SHEATH_LIFT_LEFT`]: [`grip_back`] mirrored across the body's midplane (the
/// business end down and to the right, the flat still out of the back), a proper rotation.
pub fn grip_back_left() -> Mat4 {
    let (sin, cos) = SHEATH_TILT_DEG.to_radians().sin_cos();
    Mat4::from_cols(
        glam::Vec4::new(0.0, -sin, -cos, 0.0),
        glam::Vec4::new(0.0, -cos, sin, 0.0),
        glam::Vec4::new(-1.0, 0.0, 0.0, 0.0),
        glam::Vec4::W,
    )
}

/// Where the off hand's prop is drawn sheathed (LOOK.md 6.7): as [`prop_attach_back`],
/// over the left shoulder with the mirrored grip.
pub fn prop_attach_back_left(pivots: &[Vec3; BONES], skin: &[Mat4; BONES]) -> Mat4 {
    let at = crate::rig::bone::PROP_BACK;
    skin[at] * Mat4::from_translation(pivots[at] + SHEATH_LIFT_LEFT) * grip_back_left()
}

/// The skinning matrix of every bone: `D(bone) = D(parent) · T(pivot) · R · T(−pivot)`, with
/// the body offset applied at the root. A bone missing from `mask` behaves as its parent.
pub fn skin_matrices(pivots: &[Vec3; BONES], mask: u32, pose: &Pose) -> [Mat4; BONES] {
    let mut out = [Mat4::IDENTITY; BONES];
    for b in 0..BONES {
        let parent = match PARENTS[b] {
            NO_PARENT => Mat4::from_translation(pose.offset),
            p => out[p as usize],
        };
        out[b] = if mask & (1 << b) == 0 {
            parent
        } else {
            let (q, p) = (pose.rot[b], pivots[b]);
            parent * Mat4::from_rotation_translation(q, p - q * p)
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::{self, ALL_BONES, bone};
    use gm_core::vocab::ArchetypeFrame;

    #[test]
    fn a_view_fit_turns_about_the_upright_and_moves_in_metres() {
        // The musket lies along the arm (+Y of model space after its fit of 88° about
        // glTF's Y); its view fit of −88° brings the muzzle back along +X.
        let m = view_fit(&[0.0, 0.0, 0.0, 0.0, -88.0, 0.0, 1.0]);
        let muzzle = m.transform_vector3(Vec3::Y);
        assert!(
            (muzzle - Vec3::new(88f32.to_radians().sin(), 88f32.to_radians().cos(), 0.0)).length()
                < 1e-5,
            "{muzzle}"
        );
        // A move of a metre forward in glTF (+Z) is 32 units along model +X; up (+Y) is +Z.
        let m = view_fit(&[0.0, 0.5, 1.0, 0.0, 0.0, 0.0, 2.0]);
        assert_eq!(m.transform_point3(Vec3::ZERO), Vec3::new(32.0, 0.0, 16.0));
        assert_eq!(m.transform_vector3(Vec3::X), Vec3::X * 2.0);
    }

    #[test]
    fn the_grip_is_a_rotation_that_holds_a_blade_across_the_arm() {
        let g = grip_right();
        assert!(
            (g.determinant() - 1.0).abs() < 1e-6,
            "a rotation, not a mirror"
        );
        assert!((g * g.transpose()).abs_diff_eq(Mat4::IDENTITY, 1e-6));
        // The business end (+X) points ahead of the T-pose's right arm (which runs −Y),
        // tipped a little towards the elbow; the edge (+Y) runs on away from it.
        let blade = g.transform_vector3(Vec3::X);
        assert!(
            blade.x > 0.95 && blade.y > 0.1 && blade.z.abs() < 1e-6,
            "{blade:?}"
        );
        assert!(g.transform_vector3(Vec3::Y).y < -0.95);
    }

    #[test]
    fn the_sheath_is_a_rotation_that_lays_a_blade_down_the_back() {
        let g = grip_back();
        assert!(
            (g.determinant() - 1.0).abs() < 1e-6,
            "a rotation, not a mirror"
        );
        assert!((g * g.transpose()).abs_diff_eq(Mat4::IDENTITY, 1e-6));
        // The business end runs down and to the left of the body (−Z, +Y), the flat (+Z
        // of the prop) faces out of the back (−X).
        let blade = g.transform_vector3(Vec3::X);
        assert!(
            blade.z < -0.8 && blade.y > 0.4 && blade.x.abs() < 1e-6,
            "{blade:?}"
        );
        assert!(g.transform_vector3(Vec3::Z).x < -0.99);
        // On the standing mannequin the grip is over the right shoulder, behind the body,
        // and the tip of a blade a metre long ends near the left hip, still behind it.
        let pivots = rig::rest_pivots(ArchetypeFrame::Striker);
        let skin = skin_matrices(&pivots, ALL_BONES, &Pose::REST);
        let at = prop_attach_back(&pivots, &skin);
        let grip = at.transform_point3(Vec3::ZERO);
        let tip = at.transform_point3(Vec3::X * 32.0);
        let shoulder = pivots[bone::UPPER_ARM_R];
        assert!(
            grip.z > shoulder.z && grip.y < 0.0 && grip.x < 0.0,
            "{grip:?}"
        );
        assert!(
            tip.z < pivots[bone::HIPS].z + 4.0 && tip.z > pivots[bone::THIGH_L].z - 8.0,
            "{tip:?}"
        );
        assert!(tip.y > 0.0 && (tip.x - grip.x).abs() < 1e-4, "{tip:?}");
        // Behind the pivot by SHEATH_OUT: clear of the shoulder blades.
        assert!(
            (grip.x - (pivots[bone::PROP_BACK].x - SHEATH_OUT)).abs() < 1e-4,
            "{grip:?}"
        );
    }

    #[test]
    fn a_shield_hangs_flat_on_the_back_face_out_point_down() {
        let g = grip_back_flat();
        assert!(
            (g.determinant() - 1.0).abs() < 1e-6,
            "a rotation, not a mirror"
        );
        assert!((g * g.transpose()).abs_diff_eq(Mat4::IDENTITY, 1e-6));
        assert!(
            g.transform_vector3(Vec3::Z).x < -0.99,
            "the face out of the back"
        );
        assert!(g.transform_vector3(Vec3::X).z < -0.99, "the point down");
        let pivots = rig::rest_pivots(ArchetypeFrame::Striker);
        let skin = skin_matrices(&pivots, ALL_BONES, &Pose::REST);
        let at = prop_attach_back_flat(&pivots, &skin);
        let centre = at.transform_point3(Vec3::ZERO);
        let right = prop_attach_back(&pivots, &skin).transform_point3(Vec3::ZERO);
        // On the midline, behind the blades, at the pivot's height.
        assert!(
            centre.y.abs() < 1e-4 && centre.x < right.x - 1.0,
            "{centre:?}"
        );
        assert!(
            (centre.z - pivots[bone::PROP_BACK].z).abs() < 1e-4,
            "{centre:?}"
        );
    }

    #[test]
    fn the_off_hand_sheath_is_the_mirror_image_a_little_further_out() {
        let g = grip_back_left();
        assert!(
            (g.determinant() - 1.0).abs() < 1e-6,
            "a rotation, not a mirror"
        );
        assert!((g * g.transpose()).abs_diff_eq(Mat4::IDENTITY, 1e-6));
        // The business end runs down and to the right (−Z, −Y), the flat out of the back.
        let blade = g.transform_vector3(Vec3::X);
        assert!(
            blade.z < -0.8 && blade.y < -0.4 && blade.x.abs() < 1e-6,
            "{blade:?}"
        );
        assert!(g.transform_vector3(Vec3::Z).x < -0.99);
        let pivots = rig::rest_pivots(ArchetypeFrame::Striker);
        let skin = skin_matrices(&pivots, ALL_BONES, &Pose::REST);
        let right = prop_attach_back(&pivots, &skin);
        let left = prop_attach_back_left(&pivots, &skin);
        let (gr, gl) = (
            right.transform_point3(Vec3::ZERO),
            left.transform_point3(Vec3::ZERO),
        );
        let (tr, tl) = (
            right.transform_point3(Vec3::X * 32.0),
            left.transform_point3(Vec3::X * 32.0),
        );
        // Mirror images in Y, the same height, the left one further behind; the blades cross.
        assert!(
            (gr.y + gl.y).abs() < 1e-4 && (gr.z - gl.z).abs() < 1e-4,
            "{gr:?} {gl:?}"
        );
        assert!(gl.x < gr.x - 0.5, "{gr:?} {gl:?}");
        assert!(
            (tr.y + tl.y).abs() < 1e-4 && tl.y < 0.0 && tr.y > 0.0,
            "{tr:?} {tl:?}"
        );
    }

    #[test]
    fn the_rest_pose_moves_nothing() {
        let pivots = rig::rest_pivots(ArchetypeFrame::Striker);
        for m in skin_matrices(&pivots, ALL_BONES, &Pose::REST) {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-6));
        }
    }

    #[test]
    fn a_bone_rotates_about_its_pivot_and_carries_its_children() {
        let pivots = rig::rest_pivots(ArchetypeFrame::Striker);
        let mut pose = Pose::REST;
        // Lower the left arm: +Y swings to −Z about the shoulder.
        pose.rot[bone::UPPER_ARM_L] = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let m = skin_matrices(&pivots, ALL_BONES, &pose);
        let shoulder = pivots[bone::UPPER_ARM_L];
        let elbow = pivots[bone::FOREARM_L];
        let len = (elbow - shoulder).length();
        // The shoulder stays put; the elbow hangs straight below it; the hand follows.
        assert!(
            m[bone::UPPER_ARM_L]
                .transform_point3(shoulder)
                .abs_diff_eq(shoulder, 1e-4)
        );
        let moved = m[bone::FOREARM_L].transform_point3(elbow);
        assert!(
            moved.abs_diff_eq(shoulder - Vec3::Z * len, 1e-3),
            "{moved:?}"
        );
        let hand = m[bone::HAND_L].transform_point3(pivots[bone::HAND_L]);
        assert!(hand.z < moved.z && (hand.y - shoulder.y).abs() < 1e-3);
        // The other arm and the legs are untouched.
        assert!(m[bone::UPPER_ARM_R].abs_diff_eq(Mat4::IDENTITY, 1e-6));
        assert!(m[bone::THIGH_L].abs_diff_eq(Mat4::IDENTITY, 1e-6));
    }

    #[test]
    fn an_absent_bone_follows_its_parent() {
        let pivots = rig::rest_pivots(ArchetypeFrame::Caster);
        let mut pose = Pose::REST;
        pose.rot[bone::CLAVICLE_L] = Quat::from_rotation_z(0.5);
        pose.rot[bone::CHEST] = Quat::from_rotation_z(0.2);
        let mask = ALL_BONES & !(1 << bone::CLAVICLE_L);
        let m = skin_matrices(&pivots, mask, &pose);
        assert!(m[bone::CLAVICLE_L].abs_diff_eq(m[bone::CHEST], 1e-6));
        assert!(m[bone::UPPER_ARM_L].abs_diff_eq(m[bone::CHEST], 1e-6));
    }

    #[test]
    fn blending_takes_the_short_way_and_moves_the_offset() {
        let mut a = Pose::REST;
        let mut b = Pose::REST;
        a.rot[0] = Quat::from_rotation_z(0.1);
        b.rot[0] = -Quat::from_rotation_z(0.3); // the same rotation, negated
        b.offset = Vec3::new(0.0, 0.0, -10.0);
        let mid = a.blend(&b, 0.5);
        let expect = Quat::from_rotation_z(0.2);
        assert!(mid.rot[0].abs_diff_eq(expect, 1e-3) || mid.rot[0].abs_diff_eq(-expect, 1e-3));
        assert_eq!(mid.offset.z, -5.0);
        assert_eq!(a.blend(&b, 0.0), a);
    }
}
