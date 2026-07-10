// Generated with hyprwire-scanner 0.4.3. Made with pure malice and hatred by r0chd.
// seat_v1
#[allow(
    clippy::doc_markdown,
    clippy::unwrap_used,
    clippy::wildcard_imports,
    dead_code,
    unused_imports
)]
pub mod client {
    use std::{os::fd::*, rc, sync};
    pub mod seat_manager {
        use super::*;
        pub type ProtocolImpl = super::SeatV1Impl;
        /// seat manager
        ///
        /// Interface for creating client seat connections.
        pub struct SeatManager {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for SeatManager {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for SeatManager {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for SeatManager {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for SeatManager {}
        impl std::hash::Hash for SeatManager {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `seat_manager::SeatManager`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {}
        struct SeatManagerObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for SeatManagerObjectData<D> {}
        unsafe impl<D> Sync for SeatManagerObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<seat_manager::SeatManager> + 'static,
        > hyprwire::implementation::object::ObjectData for SeatManagerObjectData<D> {
            fn dispatch(
                &self,
                __method: u32,
                __data: &[u8],
                __fds: &[i32],
                __state: &mut dyn std::any::Any,
            ) {
                let Some(__dispatch) = __state.downcast_mut::<D>() else {
                    return;
                };
                unsafe { sync::Arc::increment_strong_count(self.object) };
                let __proxy = seat_manager::SeatManager {
                    object: unsafe { sync::Arc::from_raw(self.object) },
                };
            }
        }
        impl hyprwire::Object for SeatManager {
            type Event<'a> = Event;
            type ProtocolImpl = super::SeatV1Impl;
            const NAME: &str = "seat_manager";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self::new::<D>(object)
            }
        }
        impl SeatManager {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(SeatManagerObjectData::<
                    D,
                > {
                    object: sync::Arc::as_ptr(&object),
                    _phantom: std::marker::PhantomData,
                });
                object.set_object_data(object_data);
                Self { object }
            }
            /// open the caller's seat
            ///
            /// Creates a seat object for the client's current session.
            /// The daemon sends either enabled or disabled to indicate the initial seat
            /// state.
            ///
            /// Returns a new `seat` object.
            pub fn send_open_seat<D: hyprwire::Dispatch<super::seat::Seat> + 'static>(
                &self,
            ) -> Option<super::seat::Seat> {
                let seq = self.object.call(0u32, &[]);
                let obj = self
                    .object
                    .client_sock()
                    .and_then(|sock| sock.object_for_seq(seq));
                Some(<super::seat::Seat as hyprwire::Object>::from_object::<D>(obj?))
            }
        }
    }
    pub mod seat {
        use super::*;
        pub type ProtocolImpl = super::SeatV1Impl;
        /// client seat
        ///
        /// Seat control interface based on libseat. A client opens a seat, reacts to
        /// enabled and disabled events, opens device objects while enabled, and may
        /// request session switches when supported by the daemon.
        pub struct Seat {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for Seat {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for Seat {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for Seat {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for Seat {}
        impl std::hash::Hash for Seat {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `seat::Seat`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            /// seat is active for this client
            ///
            /// Sent when the client may open and use seat devices.
            Enabled,
            /// seat is inactive for this client
            ///
            /// Sent when the client must stop using devices opened through this seat and
            /// acknowledge the state change with ack_disabled.
            Disabled,
            /// session switch became active
            ///
            /// Emitted when a session switch has taken effect.
            SessionSwitched { session: u32 },
            /// fatal seat protocol error
            ///
            /// Sent before the seat object is terminated after a fatal seat-scoped
            /// protocol error.
            SeatError { seat_error: super::super::spec::SeatError, message: String },
        }
        struct SeatObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for SeatObjectData<D> {}
        unsafe impl<D> Sync for SeatObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<seat::Seat> + 'static,
        > hyprwire::implementation::object::ObjectData for SeatObjectData<D> {
            fn dispatch(
                &self,
                __method: u32,
                __data: &[u8],
                __fds: &[i32],
                __state: &mut dyn std::any::Any,
            ) {
                let Some(__dispatch) = __state.downcast_mut::<D>() else {
                    return;
                };
                unsafe { sync::Arc::increment_strong_count(self.object) };
                let __proxy = seat::Seat {
                    object: unsafe { sync::Arc::from_raw(self.object) },
                };
                match __method {
                    0u32 => {
                        __dispatch.event(&__proxy, seat::Event::Enabled);
                    }
                    1u32 => {
                        __dispatch.event(&__proxy, seat::Event::Disabled);
                    }
                    2u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let session = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __dispatch
                            .event(
                                &__proxy,
                                seat::Event::SessionSwitched {
                                    session,
                                },
                            );
                    }
                    3u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let seat_error: super::super::spec::SeatError = unsafe {
                            std::mem::transmute::<
                                u32,
                                super::super::spec::SeatError,
                            >(
                                u32::from_le_bytes(
                                    __data[__needle..__needle + 4].try_into().unwrap(),
                                ),
                            )
                        };
                        __needle += 4;
                        __needle += 1;
                        let (__len, __vl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __vl;
                        let message = String::from_utf8_lossy(
                                &__data[__needle..__needle + __len],
                            )
                            .into_owned();
                        __needle += __len;
                        __dispatch
                            .event(
                                &__proxy,
                                seat::Event::SeatError {
                                    seat_error,
                                    message,
                                },
                            );
                    }
                    _ => {}
                }
            }
        }
        impl hyprwire::Object for Seat {
            type Event<'a> = Event;
            type ProtocolImpl = super::SeatV1Impl;
            const NAME: &str = "seat";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self::new::<D>(object)
            }
        }
        impl Seat {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(SeatObjectData::<
                    D,
                > {
                    object: sync::Arc::as_ptr(&object),
                    _phantom: std::marker::PhantomData,
                });
                object.set_object_data(object_data);
                Self { object }
            }
            /// acknowledge disabled state
            ///
            /// Acknowledges the most recent disabled event.
            pub fn send_ack_disabled(&self) {
                self.object.call(0u32, &[]);
            }
            /// open a device on this seat
            ///
            /// Opens path and returns a device object. The returned object resolves
            /// exactly once with either opened or failed.
            ///
            /// Arguments:
            /// `path`: device path
            ///
            /// Returns a new `device` object.
            pub fn send_open_device<
                D: hyprwire::Dispatch<super::device::Device> + 'static,
            >(&self, path: impl AsRef<str>) -> Option<super::device::Device> {
                let seq = self
                    .object
                    .call(
                        1u32,
                        &[
                            hyprwire::core::types::CallArg::Varchar(
                                path.as_ref().as_bytes(),
                            ),
                        ],
                    );
                let obj = self
                    .object
                    .client_sock()
                    .and_then(|sock| sock.object_for_seq(seq));
                Some(<super::device::Device as hyprwire::Object>::from_object::<D>(obj?))
            }
            /// switch to another session or VT
            ///
            /// Requests switching to the given target session. The daemon reports a seat
            /// error if switching is not supported.
            ///
            /// Arguments:
            /// `session`: target session or VT number
            pub fn send_switch_session(&self, session: u32) {
                self.object.call(2u32, &[hyprwire::core::types::CallArg::Uint(session)]);
            }
        }
    }
    pub mod device {
        use super::*;
        pub type ProtocolImpl = super::SeatV1Impl;
        /// seat device
        ///
        /// Device object created by seat.open_device. It resolves exactly once with
        /// either opened or failed. If revoked is emitted, the device is permanently
        /// unusable and the client should destroy the object.
        pub struct Device {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for Device {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for Device {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for Device {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for Device {}
        impl std::hash::Hash for Device {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `device::Device`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            Opened { device_id: u32, fd: OwnedFd, path: String },
            Failed {
                device_error: super::super::spec::DeviceError,
                errno: u32,
                message: String,
            },
            /// device revoked
            ///
            /// The fd and object must not be used again after this event.
            Revoked,
        }
        struct DeviceObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for DeviceObjectData<D> {}
        unsafe impl<D> Sync for DeviceObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<device::Device> + 'static,
        > hyprwire::implementation::object::ObjectData for DeviceObjectData<D> {
            fn dispatch(
                &self,
                __method: u32,
                __data: &[u8],
                __fds: &[i32],
                __state: &mut dyn std::any::Any,
            ) {
                let Some(__dispatch) = __state.downcast_mut::<D>() else {
                    return;
                };
                unsafe { sync::Arc::increment_strong_count(self.object) };
                let __proxy = device::Device {
                    object: unsafe { sync::Arc::from_raw(self.object) },
                };
                match __method {
                    0u32 => {
                        let mut __needle: usize = 0;
                        let mut __fds_iter = __fds.iter();
                        __needle += 1;
                        let device_id = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __needle += 1;
                        let fd = unsafe {
                            OwnedFd::from_raw_fd(*__fds_iter.next().unwrap())
                        };
                        __needle += 1;
                        let (__len, __vl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __vl;
                        let path = String::from_utf8_lossy(
                                &__data[__needle..__needle + __len],
                            )
                            .into_owned();
                        __needle += __len;
                        __dispatch
                            .event(
                                &__proxy,
                                device::Event::Opened {
                                    device_id,
                                    fd,
                                    path,
                                },
                            );
                    }
                    1u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let device_error: super::super::spec::DeviceError = unsafe {
                            std::mem::transmute::<
                                u32,
                                super::super::spec::DeviceError,
                            >(
                                u32::from_le_bytes(
                                    __data[__needle..__needle + 4].try_into().unwrap(),
                                ),
                            )
                        };
                        __needle += 4;
                        __needle += 1;
                        let errno = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __needle += 1;
                        let (__len, __vl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __vl;
                        let message = String::from_utf8_lossy(
                                &__data[__needle..__needle + __len],
                            )
                            .into_owned();
                        __needle += __len;
                        __dispatch
                            .event(
                                &__proxy,
                                device::Event::Failed {
                                    device_error,
                                    errno,
                                    message,
                                },
                            );
                    }
                    2u32 => {
                        __dispatch.event(&__proxy, device::Event::Revoked);
                    }
                    _ => {}
                }
            }
        }
        impl hyprwire::Object for Device {
            type Event<'a> = Event;
            type ProtocolImpl = super::SeatV1Impl;
            const NAME: &str = "device";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self::new::<D>(object)
            }
        }
        impl Device {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(DeviceObjectData::<
                    D,
                > {
                    object: sync::Arc::as_ptr(&object),
                    _phantom: std::marker::PhantomData,
                });
                object.set_object_data(object_data);
                Self { object }
            }
        }
    }
    #[derive(Default, Clone)]
    pub struct SeatV1Impl {
        protocol: super::spec::SeatV1ProtocolSpec,
    }
    impl hyprwire::implementation::client::ProtocolImplementations for SeatV1Impl {
        fn new() -> Self {
            Self::default()
        }
        fn protocol_spec() -> Box<dyn hyprwire::core::types::ProtocolSpec> {
            Box::new(super::spec::SeatV1ProtocolSpec::default())
        }
        fn spec_name() -> &'static str {
            "seat_v1"
        }
        fn protocol(&self) -> &dyn hyprwire::core::types::ProtocolSpec {
            &self.protocol
        }
        fn implementation(
            &self,
        ) -> &[hyprwire::implementation::client::ObjectImplementation<'_>] {
            &[]
        }
    }
}
#[allow(clippy::doc_markdown, dead_code)]
mod spec {
    #[non_exhaustive]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u32)]
    pub enum DeviceError {
        SeatDisabled = 0u32,
        OpenFailed = 1u32,
    }
    #[non_exhaustive]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u32)]
    pub enum SeatError {
        InvalidAckDisabled = 0u32,
        AlreadyDisabled = 1u32,
        InvalidSwitchSession = 2u32,
    }
    pub struct SeatManagerSpec {
        c2s_methods: &'static [hyprwire::core::types::Method],
        s2c_methods: &'static [hyprwire::core::types::Method],
    }
    static SEAT_MANAGER: std::sync::LazyLock<
        std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>,
    > = std::sync::LazyLock::new(|| std::sync::Arc::new(SeatManagerSpec {
        c2s_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[],
                returns_type: "seat",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 1u32,
                params: &[],
                returns_type: "",
                since: 0,
                destructor: true,
            },
        ],
        s2c_methods: &[],
    }));
    impl hyprwire::core::types::ProtocolObjectSpec for SeatManagerSpec {
        fn object_name(&self) -> &'static str {
            "seat_manager"
        }
        fn c2s(&self) -> &[hyprwire::core::types::Method] {
            self.c2s_methods
        }
        fn s2c(&self) -> &[hyprwire::core::types::Method] {
            self.s2c_methods
        }
    }
    pub struct SeatSpec {
        c2s_methods: &'static [hyprwire::core::types::Method],
        s2c_methods: &'static [hyprwire::core::types::Method],
    }
    static SEAT: std::sync::LazyLock<
        std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>,
    > = std::sync::LazyLock::new(|| std::sync::Arc::new(SeatSpec {
        c2s_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 1u32,
                params: &[hyprwire::core::types::MessageMagic::TypeVarchar as u8],
                returns_type: "device",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 2u32,
                params: &[hyprwire::core::types::MessageMagic::TypeUint as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 3u32,
                params: &[],
                returns_type: "",
                since: 0,
                destructor: true,
            },
        ],
        s2c_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 1u32,
                params: &[],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 2u32,
                params: &[hyprwire::core::types::MessageMagic::TypeUint as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 3u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
        ],
    }));
    impl hyprwire::core::types::ProtocolObjectSpec for SeatSpec {
        fn object_name(&self) -> &'static str {
            "seat"
        }
        fn c2s(&self) -> &[hyprwire::core::types::Method] {
            self.c2s_methods
        }
        fn s2c(&self) -> &[hyprwire::core::types::Method] {
            self.s2c_methods
        }
    }
    pub struct DeviceSpec {
        c2s_methods: &'static [hyprwire::core::types::Method],
        s2c_methods: &'static [hyprwire::core::types::Method],
    }
    static DEVICE: std::sync::LazyLock<
        std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>,
    > = std::sync::LazyLock::new(|| std::sync::Arc::new(DeviceSpec {
        c2s_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[],
                returns_type: "",
                since: 0,
                destructor: true,
            },
        ],
        s2c_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                    hyprwire::core::types::MessageMagic::TypeFd as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 1u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 2u32,
                params: &[],
                returns_type: "",
                since: 0,
                destructor: false,
            },
        ],
    }));
    impl hyprwire::core::types::ProtocolObjectSpec for DeviceSpec {
        fn object_name(&self) -> &'static str {
            "device"
        }
        fn c2s(&self) -> &[hyprwire::core::types::Method] {
            self.c2s_methods
        }
        fn s2c(&self) -> &[hyprwire::core::types::Method] {
            self.s2c_methods
        }
    }
    #[derive(Clone)]
    pub struct SeatV1ProtocolSpec {
        objects: [std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>; 3usize],
    }
    impl Default for SeatV1ProtocolSpec {
        fn default() -> Self {
            Self {
                objects: [
                    std::sync::Arc::clone(&SEAT_MANAGER),
                    std::sync::Arc::clone(&SEAT),
                    std::sync::Arc::clone(&DEVICE),
                ],
            }
        }
    }
    impl hyprwire::core::types::ProtocolSpec for SeatV1ProtocolSpec {
        fn spec_name(&self) -> &'static str {
            "seat_v1"
        }
        fn spec_ver(&self) -> u32 {
            1u32
        }
        fn objects(
            &self,
        ) -> &[std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>] {
            &self.objects
        }
    }
}
