use crate::scheduler::{Scheduler, SCHEDULER};

pub fn exit() {
    log::info!("Killing active process (pid: {})", Scheduler::active_pid(&*SCHEDULER));
    Scheduler::mark_active_dead(&*SCHEDULER);
}
