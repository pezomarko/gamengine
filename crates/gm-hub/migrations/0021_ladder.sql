-- The ladder (SKILLS.md 5): a new character earns its second move at the yard, its trick at
-- the dungeon's gate and its spell in its hall. The characters of before keep what they
-- had: they pass the three steps here. The rite is not given: the Warden wakes the Essence.
insert into trials (character_id, trial, zone, secs)
    select c.id, t.trial, 'ladder', 0
    from characters c cross join (values ('yard'), ('gate'), ('hall')) as t (trial)
on conflict do nothing;
