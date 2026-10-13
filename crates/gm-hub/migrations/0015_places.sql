-- The body's places and the trinkets (EQUIPMENT.md 5, 2026-10-13): beside the two hands
-- and the chest (`armour`), the head, the hands, the legs, the feet, the back, the
-- shoulders, the neck and two fingers. The rule stays: one item in each place, an item
-- in one place only.
alter table worn drop constraint worn_slot_check;
alter table worn add constraint worn_slot_check check (slot in (
    'weapon', 'off', 'armour', 'head', 'hands', 'legs', 'feet', 'back', 'shoulders',
    'neck', 'ring', 'ring2'));
