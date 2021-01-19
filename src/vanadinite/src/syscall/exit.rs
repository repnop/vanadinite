use crate::scheduler::Scheduler;

pub fn exit() {
    log::info!("Killing active process (pid: {})", Scheduler::active_pid());
    Scheduler::mark_active_dead();
}
