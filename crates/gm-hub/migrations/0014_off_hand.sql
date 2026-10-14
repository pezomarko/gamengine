-- The off hand (ITEMS.md 2, 2026-10-13): a third place beside the weapon and the armour,
-- for a shield or a one-handed weapon worn in the left hand. The rule stays: one item in
-- each place, an item in one place only.
alter table worn drop constraint worn_slot_check;
alter table worn add constraint worn_slot_check check (slot in ('weapon', 'off', 'armour'));
