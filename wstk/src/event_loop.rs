use futures::channel::{mpsc, oneshot};
use smithay_client_toolkit::reexports::client::{
    EventQueue, Interface, Main, MessageGroup, Proxy, ProxyMap,
};
use std::sync::Arc;

/// Enables Wayland event dispatch on the glib event loop. Requires 'static :(
pub fn glib_add_wayland(event_queue: &'static mut EventQueue) {
    let fd = event_queue.display().get_connection_fd();
    glib::source::unix_fd_add_local(fd, glib::IOCondition::IN, move |_fd, _ioc| {
        event_queue.dispatch(&mut (), |_, _, _| {}).unwrap();
        glib::Continue(true)
    });
}

/// Creates a mpsc channel for a Wayland object's events.
pub fn wayland_event_chan<I>(obj: &Main<I>) -> mpsc::UnboundedReceiver<Arc<I::Event>>
where
    I: Interface + AsRef<Proxy<I>> + From<Proxy<I>> + Sync,
    I::Event: MessageGroup<Map = ProxyMap>,
{
    let (tx, rx) = mpsc::unbounded();
    obj.quick_assign(move |_, event, _| {
        tx.unbounded_send(Arc::new(event)).unwrap();
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
