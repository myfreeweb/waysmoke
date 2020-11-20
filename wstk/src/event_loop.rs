use futures::channel::{mpsc, oneshot};
use smithay_client_toolkit::{
    reexports::client::{protocol::wl_seat, Attached, EventQueue, Interface, Main, MessageGroup, Proxy, ProxyMap},
    seat,
};

/// Enables Wayland event dispatch on the glib event loop. Requires 'static :(
pub fn glib_add_wayland(event_queue: &'static mut EventQueue) {
    let fd = event_queue.display().get_connection_fd();
    glib::source::unix_fd_add_local(fd, glib::IOCondition::IN, move |_fd, _ioc| {
        if let Some(guard) = event_queue.prepare_read() {
            if let Err(e) = event_queue.display().flush() {
                eprintln!("Error flushing the wayland socket: {:?}", e);
            }

            if let Err(e) = guard.read_events() {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    eprintln!("Reading from the wayland socket would block!");
                    return glib::Continue(true);
                } else {
                    eprintln!("Error reading from the wayland socket: {:?}", e);
                }
            }
        }
        event_queue.dispatch_pending(&mut (), |_, _, _| {}).unwrap();
        glib::Continue(true)
    });
}

/// Creates a mpsc channel for a Wayland object's events.
pub fn wayland_event_chan<I>(obj: &Main<I>) -> mpsc::UnboundedReceiver<I::Event>
where
    I: Interface + AsRef<Proxy<I>> + From<Proxy<I>> + Sync,
    I::Event: MessageGroup<Map = ProxyMap>,
{
    let (tx, rx) = mpsc::unbounded();
    obj.quick_assign(move |_, event, _| {
        if let Err(e) = tx.unbounded_send(event) {
            if !e.is_disconnected() {
                panic!("Unexpected send error {:?}", e)
            }
        }
    });
    rx
}

/// Creates a oneshot channel for a Wayland object's events, intended for WlCallback.
pub fn wayland_event_chan_oneshot<I>(obj: &Main<I>) -> oneshot::Receiver<I::Event>
where
    I: Interface + AsRef<Proxy<I>> + From<Proxy<I>> + Sync,
    I::Event: MessageGroup<Map = ProxyMap>,
{
    let (tx, rx) = oneshot::channel();
    // would be great to have a quick_assign with FnOnce
    let txc = std::cell::Cell::new(Some(tx));
    obj.quick_assign(move |_, event, _| {
        if let Ok(_) = txc.take().unwrap().send(event) {
        } else {
            eprintln!("Event-to-oneshot-channel send with no receiver?");
        }
        ()
    });
    rx
}

/// Creates a mpsc channel for a Wayland object's events.
pub fn wayland_keyboard_chan(seat: &Attached<wl_seat::WlSeat>) -> mpsc::UnboundedReceiver<seat::keyboard::Event> {
    let (tx, rx) = mpsc::unbounded();
    seat::keyboard::map_keyboard(seat, None, move |event, _, _| {
        if let Err(e) = tx.unbounded_send(event) {
            if !e.is_disconnected() {
                panic!("Unexpected send error {:?}", e)
            }
        }
    })
    .unwrap();
    rx
}
