-- The templates that went with the six places (EQUIPMENT.md 10.8, migration 0016) leave
-- the game (2026-10-14, the director: "remove removed items from game"): the pauldrons,
-- the bracers, the boots, the shoulder plates and the amulet, wherever they are (a bag, a
-- stall's counter, an offer), with their components and listings; each goes to nowhere in
-- the item log under `retire`, as a consumed stack does. Nothing wears them (0016 took
-- them off). Repeatable: a second run finds nothing.
create temporary table retired as
    select i.id, i.holder_id from items i
    where i.template in ('pauldrons', 'bracers', 'boots', 'plate_pauldrons', 'amulet');
insert into item_moves (item_id, from_holder, to_holder, reason, ref)
    select id, holder_id, null, 'retire', 0 from retired;
delete from worn where item_id in (select id from retired);
delete from listings where item_id in (select id from retired);
delete from trade_items where item_id in (select id from retired);
delete from item_components where item_id in (select id from retired);
delete from items where id in (select id from retired);
drop table retired;
