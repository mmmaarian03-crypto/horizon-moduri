// Polymorph inside the guests' crafting (`game/lan_crafting.rs`): a child of `lan_crafting`, so it reaches
// the guest's open crafting table menu (`GuestCrafting` keeps it private).
use super::{Game, LanHost};

impl Game<'_> {
    /// The guest's open crafting table makes its result again (from the choice this thread's file holds
    /// now): the menu stays open between clicks, and its last result may come from another choice.
    pub(in crate::game) fn polymorph_recompute_table(&mut self, host: &mut LanHost, conn: u32) {
        let Some((_, _, menu, _)) = host.play.get_mut(&conn).and_then(|p| p.crafting.table.as_mut()) else {
            return;
        };
        let env = self.menu_env();
        // Slot 1 is the grid's first cell: setting it again recomputes (`Menu::put` on the grid).
        let first = menu.slot(1).cloned();
        menu.set_slot(&env, 1, first);
    }
}
