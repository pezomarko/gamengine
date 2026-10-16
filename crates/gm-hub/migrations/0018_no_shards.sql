-- The shard layer leaves the game (2026-10-16, GEAR.md 6; the director: "remove shards and
-- bleeds completely ... bleed is bound to skill not the weapon. Weapon craft is therefore
-- simpler"). A crafted item loses its shard row (its edge is the smaller for it); a loose
-- shard, a component item whose one row is a shard, goes to nowhere in the item log under
-- `retire`, wherever it is (a bag, a stall's counter, an offer), as the retired templates
-- did (0017). The layer check no longer admits a shard. Repeatable.
create temporary table shards as
    select i.id, i.holder_id from items i
    where i.template = 'component'
      and exists (select 1 from item_components c where c.item_id = i.id and c.layer = 'shard');
insert into item_moves (item_id, from_holder, to_holder, reason, ref)
    select id, holder_id, null, 'retire', 0 from shards;
delete from worn where item_id in (select id from shards);
delete from listings where item_id in (select id from shards);
delete from trade_items where item_id in (select id from shards);
delete from item_components where item_id in (select id from shards);
delete from items where id in (select id from shards);
drop table shards;
delete from item_components where layer = 'shard';
alter table item_components drop constraint if exists item_components_layer_check;
alter table item_components add constraint item_components_layer_check
    check (layer in ('core', 'catalyst', 'frame', 'gem'));
