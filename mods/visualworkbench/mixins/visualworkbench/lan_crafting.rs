// Visual Workbench inside the guests' crafting (`game/lan_crafting.rs`): a server's guest (single player's
// window included) at a crafting table. A child of `lan_crafting`, so it reaches the guest's open table
// menu (`GuestCrafting` keeps it private). The table's contents are `game::visualworkbench`'s.
use super::{Game, LanHost};
use horizon_client::items::ItemStack;

impl Game<'_> {
    /// HEAD on `lan_send_table_content` (the guest's table menu opened, or changed by a click, the recipe
    /// book, JEI): a new menu takes the table's grid (`ModCraftingMenu` over the block entity), a held one
    /// is copied into the table.
    pub(in crate::game) fn visualworkbench_guest_content(&mut self, host: &mut LanHost, conn: u32) {
        let Some((window, _, menu, pos)) = host.play.get_mut(&conn).and_then(|p| p.crafting.table.as_mut()) else {
            return;
        };
        let (window, pos) = (*window, *pos);
        let grid: Vec<Option<ItemStack>> = (1..10).map(|s| menu.slot(s).cloned()).collect();
        if self.visualworkbench_guest_bound(conn) == Some((pos, window)) {
            let result = menu.slot(0).cloned();
            self.visualworkbench_mirror_guest(conn, &grid, result);
            return;
        }
        let empty = grid.iter().all(Option::is_none);
        if let Some(items) = self.visualworkbench_bind_guest(conn, pos, window, empty) {
            let env = self.menu_env();
            for (i, item) in items.into_iter().enumerate() {
                menu.set_slot(&env, 1 + i, item);
            }
        }
    }

    /// HEAD on `lan_crafting_closed` (window `None` = whichever table) and `lan_crafting_left`: a held
    /// table's grid stays in the table; the guest's menu is emptied first, so nothing goes back to him.
    pub(in crate::game) fn visualworkbench_guest_closed(&mut self, host: &mut LanHost, conn: u32, window: Option<i32>) {
        let Some(bound) = self.visualworkbench_guest_bound(conn) else {
            return;
        };
        let Some((w, _, menu, pos)) = host.play.get_mut(&conn).and_then(|p| p.crafting.table.as_mut()) else {
            // No table menu any more: nothing to keep.
            self.visualworkbench_unbind_guest(conn);
            return;
        };
        if window.is_some_and(|x| x != *w) {
            return;
        }
        if bound != (*pos, *w) {
            self.visualworkbench_unbind_guest(conn);
            return;
        }
        let env = self.menu_env();
        let grid: Vec<Option<ItemStack>> = (1..10).map(|s| menu.slot(s).cloned()).collect();
        let result = menu.slot(0).cloned();
        for s in 1..10 {
            menu.set_slot(&env, s, None);
        }
        self.visualworkbench_stash_guest(conn, &grid, result);
    }

    /// HEAD on `tick_guests` (a server's level in place): every held table of this level is copied into the
    /// table; one whose block is gone keeps the grid (spilled by the next block entity tick) and the guest's
    /// menu is emptied before `lan_tables_tick` closes it. Then the clients get the tables that changed.
    pub(in crate::game) fn visualworkbench_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        for (conn, pos, window) in self.visualworkbench_guests_here() {
            let open = host
                .play
                .get_mut(&conn)
                .and_then(|p| p.crafting.table.as_mut())
                .filter(|t| t.0 == window && t.3 == pos);
            let Some((_, _, menu, _)) = open else {
                // Closed by a path this mod does not see: the vanilla close gave the grid back to the guest.
                log::info!("visualworkbench: guest {conn}'s table at {pos:?} closed with its grid given back");
                self.visualworkbench_forget_guest(conn);
                continue;
            };
            let grid: Vec<Option<ItemStack>> = (1..10).map(|s| menu.slot(s).cloned()).collect();
            let result = menu.slot(0).cloned();
            if self.world.loaded(pos[0], pos[2]) && !self.visualworkbench_is_table(pos) {
                let env = self.menu_env();
                for s in 1..10 {
                    menu.set_slot(&env, s, None);
                }
                self.visualworkbench_stash_guest(conn, &grid, result);
            } else {
                self.visualworkbench_mirror_guest(conn, &grid, result);
            }
        }
        self.visualworkbench_send_tables(host);
    }
}
