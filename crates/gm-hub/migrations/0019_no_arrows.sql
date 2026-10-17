-- The arrow stack leaves the game (2026-10-17, the director: "arrows we can make unlimited
-- but rate limited by stamina"): the bow's draw costs stamina and nothing else, so the
-- `arrow` template of 2026-10-16 (GEAR.md 7) is retired wherever it is (a bag, a stall's
-- counter, an offer), with its listings; each goes to nowhere in the item log under
-- `retire`, as a consumed stack does (migration 0017). Repeatable: a second run finds
-- nothing.
create temporary table retired as
    select i.id, i.holder_id from items i where i.template = 'arrow';
insert into item_moves (item_id, from_holder, to_holder, reason, ref)
    select id, holder_id, null, 'retire', 0 from retired;
delete from worn where item_id in (select id from retired);
delete from listings where item_id in (select id from retired);
delete from trade_items where item_id in (select id from retired);
delete from item_components where item_id in (select id from retired);
delete from items where id in (select id from retired);
drop table retired;
