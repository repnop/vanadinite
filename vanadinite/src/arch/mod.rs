#[cfg(feature = "virt")]
pub mod virt;

pub enum ExitStatus<'a> {
    Ok,
    Error(&'a dyn core::fmt::Display),
}

#[cfg(feature = "virt")]
pub fn exit(status: ExitStatus) -> ! {
    let exit_status = match status {
        ExitStatus::Ok => virt::ExitStatus::Pass,
        ExitStatus::Error(msg) => {
            log::error!("Exiting kernel, reason: {}", msg);
            virt::ExitStatus::Fail(1)
        }
    };

    virt::exit(exit_status)
}

#[cfg(not(feature = "virt"))]
pub fn exit(_: ExitStatus) -> ! {
    // FIXME: do print here
    loop {}
}
