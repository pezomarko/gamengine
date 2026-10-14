-- Six places (EQUIPMENT.md 10.8, 2026-10-14): the two hands, the body (`armour`, the
-- chest), the head, the legs and one accessory, a ring or a cape. What was worn on the
-- hands, the feet, the shoulders, the neck and the other finger comes off (it stays in
-- the bag, like anything taken off); a ring or a cape worn becomes the accessory, the
-- ring first. The rule stays: one item in each place, an item in one place only.
delete from worn where slot in ('hands', 'feet', 'shoulders', 'neck', 'ring2');
delete from worn w where w.slot = 'back'
    and exists (select 1 from worn r where r.character_id = w.character_id and r.slot = 'ring');
update worn set slot = 'accessory' where slot in ('ring', 'back');
alter table worn drop constraint worn_slot_check;
alter table worn add constraint worn_slot_check check (slot in (
    'weapon', 'off', 'armour', 'head', 'legs', 'accessory'));
