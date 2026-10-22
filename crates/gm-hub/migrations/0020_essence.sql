-- The Essence (SKILLS.md 4): the form the rite woke in a character, and the item it sits
-- in. An item that leaves the game takes nothing with it: the Essence stays, sitting
-- nowhere, until the smith infuses it into another weapon of its form.
alter table characters add column essence_form text;
alter table characters add column essence_item bigint references items(id) on delete set null;
