use futures::{
    channel::{mpsc, oneshot},
    future::FusedFuture,
    prelude::*,
    task,
};
use smithay_client_toolkit::{
    reexports::client::{
        protocol::{wl_keyboard, wl_seat},
        Attached, EventQueue, Interface, Main, MessageGroup, Proxy, ProxyMap,
    },
    seat,
};
use std::{marker::Unpin, pin::Pin};

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

/// Wayland proxy wrapper that provides an async channel for the object's events,
/// and can run the object's destructor on Drop.
pub struct AsyncMain<I>
where
    I: Interface + AsRef<Proxy<I>> + From<Proxy<I>> + Sync,
    I::Event: MessageGroup<Map = ProxyMap>,
{
    main: Main<I>,
    rx: mpsc::UnboundedReceiver<I::Event>,
    destructor: Option<fn(&Main<I>) -> ()>,
}

impl<I> Drop for AsyncMain<I>
where
    I: Interface + AsRef<Proxy<I>> + From<Proxy<I>> + Sync,
    I::Event: MessageGroup<Map = ProxyMap>,
{
    fn drop(&mut self) {
        // XXX: https://github.com/Smithay/wayland-rs/issues/358
        self.main.quick_assign(|_, _, _| ());
        if let Some(d) = self.destructor.take() {
            d(&self.main);
        }
    }
}

impl<I> std::ops::Deref for AsyncMain<I>
where
    I: Interface + AsRef<Proxy<I>> + Into<Proxy<I>> + From<Proxy<I>> + Sync,
    I::Event: MessageGroup<Map = ProxyMap>,
{
    type Target = Attached<I>;

    fn deref(&self) -> &Self::Target {
        self.main.deref()
    }
}

impl<I> AsyncMain<I>
where
    I: Interface + AsRef<Proxy<I>> + From<Proxy<I>> + Sync,
    I::Event: MessageGroup<Map = ProxyMap>,
{
    pub fn new(main: Main<I>, destructor: Option<fn(&Main<I>) -> ()>) -> AsyncMain<I> {
        let (tx, rx) = mpsc::unbounded();
        main.quick_assign(move |_, event, _| {
            if let Err(e) = tx.unbounded_send(event) {
                if !e.is_disconnected() {
                    panic!("Unexpected send error {:?}", e)
                }
            }
        });
        AsyncMain { main, rx, destructor }
    }

    pub fn next(&mut self) -> impl FusedFuture<Output = I::Event> + '_ {
        self.rx.select_next_some()
    }
}

/// Wrapper for using a maybe-nonexistent future in a select! invocation
pub struct MaybeFuture<F>(Option<F>);

impl<F> MaybeFuture<F> {
    pub fn new(f: Option<F>) -> MaybeFuture<F> {
        MaybeFuture(f)
    }
}

impl<F: Future + Unpin> Future for MaybeFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        // XXX: unchecked should be fine here, is it faster than Unpin?
        if let Some(ref mut f) = self.get_mut().0 {
            Future::poll(Pin::new(f), cx)
        } else {
            task::Poll::Pending
        }
    }
}

impl<F: FusedFuture + Unpin> FusedFuture for MaybeFuture<F> {
    fn is_terminated(&self) -> bool {
        if let Some(ref f) = self.0 {
            f.is_terminated()
        } else {
            true
        }
    }
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
pub fn wayland_keyboard_chan(
    seat: &Attached<wl_seat::WlSeat>,
) -> (
    Main<wl_keyboard::WlKeyboard>,
    mpsc::UnboundedReceiver<seat::keyboard::Event>,
) {
    let (tx, rx) = mpsc::unbounded();
    (
        seat::keyboard::map_keyboard(seat, None, move |event, _, _| {
            if let Err(e) = tx.unbounded_send(event) {
                if !e.is_disconnected() {
                    panic!("Unexpected send error {:?}", e)
                }
            }
        })
        .unwrap(),
        rx,
    )
}
