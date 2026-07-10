// Generated with hyprwire-scanner 0.4.3. Made with pure malice and hatred by r0chd.
// test_protocol_v1
/*
 This protocol's author copyright notice is:

    I eat paint

*/

#[allow(
    clippy::doc_markdown,
    clippy::non_send_fields_in_send_ty,
    clippy::unwrap_used,
    clippy::wildcard_imports,
    dead_code,
    unused_imports
)]
pub mod server {
    use std::{os::fd::*, rc, sync};
    pub mod my_manager_v1 {
        use super::*;
        /// manager object
        ///
        /// This object is an example manager object for the protocol
        pub struct MyManagerV1 {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for MyManagerV1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for MyManagerV1 {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for MyManagerV1 {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for MyManagerV1 {}
        impl std::hash::Hash for MyManagerV1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `my_manager_v1::MyManagerV1`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            /// Send a text message
            ///
            /// Sends a text message to the server
            SendMessage { message: String },
            /// Send a text message
            ///
            /// Sends a fd message to the server
            SendMessageFd { message: OwnedFd },
            /// Send a text message
            ///
            /// Sends a fd message to the server
            SendMessageArrayFd { message: Vec<OwnedFd> },
            /// Send a text array message
            ///
            /// Sends an array message to the server
            SendMessageArray { message: Vec<String> },
            /// Send an uint array message
            ///
            /// Sends an array message to the server
            SendMessageArrayUint { message: Vec<u32> },
            /// Make a test object
            ///
            /// Makes a test object
            MakeObject { my_object_v1: super::my_object_v1::MyObjectV1 },
        }
        struct MyManagerV1ObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for MyManagerV1ObjectData<D> {}
        unsafe impl<D> Sync for MyManagerV1ObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<my_manager_v1::MyManagerV1>
                + hyprwire::Dispatch<super::my_object_v1::MyObjectV1> + 'static,
        > hyprwire::implementation::object::ObjectData for MyManagerV1ObjectData<D> {
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
                let __proxy = my_manager_v1::MyManagerV1 {
                    object: unsafe { sync::Arc::from_raw(self.object) },
                };
                match __method {
                    0u32 => {
                        let mut __needle: usize = 0;
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
                                my_manager_v1::Event::SendMessage {
                                    message,
                                },
                            );
                    }
                    1u32 => {
                        let mut __needle: usize = 0;
                        let mut __fds_iter = __fds.iter();
                        __needle += 1;
                        let message = unsafe {
                            OwnedFd::from_raw_fd(*__fds_iter.next().unwrap())
                        };
                        __dispatch
                            .event(
                                &__proxy,
                                my_manager_v1::Event::SendMessageFd {
                                    message,
                                },
                            );
                    }
                    2u32 => {
                        let mut __needle: usize = 0;
                        let mut __fds_iter = __fds.iter();
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut message = Vec::with_capacity(__count);
                        for &__fd in __fds_iter.by_ref().take(__count) {
                            message.push(unsafe { OwnedFd::from_raw_fd(__fd) });
                        }
                        __dispatch
                            .event(
                                &__proxy,
                                my_manager_v1::Event::SendMessageArrayFd {
                                    message,
                                },
                            );
                    }
                    3u32 => {
                        let mut __needle: usize = 0;
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut message = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            let (__slen, __svl) = hyprwire::core::message::parse_var_int(
                                __data,
                                __needle,
                            );
                            __needle += __svl;
                            message
                                .push(
                                    String::from_utf8_lossy(
                                            &__data[__needle..__needle + __slen],
                                        )
                                        .into_owned(),
                                );
                            __needle += __slen;
                        }
                        __dispatch
                            .event(
                                &__proxy,
                                my_manager_v1::Event::SendMessageArray {
                                    message,
                                },
                            );
                    }
                    4u32 => {
                        let mut __needle: usize = 0;
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut message = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            message
                                .push(
                                    u32::from_le_bytes(
                                        __data[__needle..__needle + 4].try_into().unwrap(),
                                    ),
                                );
                            __needle += 4;
                        }
                        __dispatch
                            .event(
                                &__proxy,
                                my_manager_v1::Event::SendMessageArrayUint {
                                    message,
                                },
                            );
                    }
                    5u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let __seq = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        let Some(__raw_obj) = __proxy
                            .object
                            .create_object("my_object_v1", __seq) else {
                            return;
                        };
                        let my_object_v1 = super::my_object_v1::MyObjectV1::new::<
                            D,
                        >(__raw_obj);
                        __dispatch
                            .event(
                                &__proxy,
                                my_manager_v1::Event::MakeObject {
                                    my_object_v1,
                                },
                            );
                    }
                    _ => {}
                }
            }
        }
        impl hyprwire::Object for MyManagerV1 {
            type Event<'a> = Event;
            type ProtocolImpl = ();
            const NAME: &str = "my_manager_v1";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self { object }
            }
        }
        impl MyManagerV1 {
            pub fn new<
                D: hyprwire::Dispatch<Self>
                    + hyprwire::Dispatch<super::my_object_v1::MyObjectV1> + 'static,
            >(object: sync::Arc<dyn hyprwire::implementation::object::Object>) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(MyManagerV1ObjectData::<
                    D,
                > {
                    object: sync::Arc::as_ptr(&object),
                    _phantom: std::marker::PhantomData,
                });
                object.set_object_data(object_data);
                Self { object }
            }
            pub fn error(&self, error_id: u32, error_msg: impl AsRef<str>) {
                self.object.error(error_id, error_msg.as_ref());
            }
            pub fn client(&self) -> Option<hyprwire::server::ServerClient> {
                self.object.server_client()
            }
            /// Send a text message
            ///
            /// Sends a text message to the client
            ///
            /// Arguments:
            /// `message`: message
            pub fn send_send_message(&self, message: impl AsRef<str>) {
                self.object
                    .call(
                        0u32,
                        &[
                            hyprwire::core::types::CallArg::Varchar(
                                message.as_ref().as_bytes(),
                            ),
                        ],
                    );
            }
            /// Receive an uint array message
            ///
            /// Receives an array message to the server
            ///
            /// Arguments:
            /// `message`: message
            pub fn send_recv_message_array_uint(&self, message: &[u32]) {
                self.object
                    .call(1u32, &[hyprwire::core::types::CallArg::UintArray(message)]);
            }
        }
    }
    pub mod my_object_v1 {
        use super::*;
        /// test object
        ///
        /// This object is an example object for the protocol
        pub struct MyObjectV1 {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for MyObjectV1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for MyObjectV1 {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for MyObjectV1 {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for MyObjectV1 {}
        impl std::hash::Hash for MyObjectV1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `my_object_v1::MyObjectV1`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            /// Send a text message
            ///
            /// Sends a text message to the server
            SendMessage { message: String },
            /// Send an enum message
            ///
            /// Sends an enum message to the server
            SendEnum { message: super::super::spec::MyEnum },
            /// Send a text message
            ///
            /// Sends a text message to the server
            Destroy,
            /// Make a test object
            ///
            /// Makes a test object from a test object
            MakeObject { my_object_v1: super::my_object_v1::MyObjectV1 },
        }
        struct MyObjectV1ObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for MyObjectV1ObjectData<D> {}
        unsafe impl<D> Sync for MyObjectV1ObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<my_object_v1::MyObjectV1> + 'static,
        > hyprwire::implementation::object::ObjectData for MyObjectV1ObjectData<D> {
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
                let __proxy = my_object_v1::MyObjectV1 {
                    object: unsafe { sync::Arc::from_raw(self.object) },
                };
                match __method {
                    0u32 => {
                        let mut __needle: usize = 0;
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
                                my_object_v1::Event::SendMessage {
                                    message,
                                },
                            );
                    }
                    1u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let message: super::super::spec::MyEnum = unsafe {
                            std::mem::transmute::<
                                u32,
                                super::super::spec::MyEnum,
                            >(
                                u32::from_le_bytes(
                                    __data[__needle..__needle + 4].try_into().unwrap(),
                                ),
                            )
                        };
                        __needle += 4;
                        __dispatch
                            .event(
                                &__proxy,
                                my_object_v1::Event::SendEnum {
                                    message,
                                },
                            );
                    }
                    2u32 => {
                        __dispatch.event(&__proxy, my_object_v1::Event::Destroy);
                    }
                    3u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let __seq = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        let Some(__raw_obj) = __proxy
                            .object
                            .create_object("my_object_v1", __seq) else {
                            return;
                        };
                        let my_object_v1 = super::my_object_v1::MyObjectV1::new::<
                            D,
                        >(__raw_obj);
                        __dispatch
                            .event(
                                &__proxy,
                                my_object_v1::Event::MakeObject {
                                    my_object_v1,
                                },
                            );
                    }
                    _ => {}
                }
            }
        }
        impl hyprwire::Object for MyObjectV1 {
            type Event<'a> = Event;
            type ProtocolImpl = ();
            const NAME: &str = "my_object_v1";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self { object }
            }
        }
        impl MyObjectV1 {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(MyObjectV1ObjectData::<
                    D,
                > {
                    object: sync::Arc::as_ptr(&object),
                    _phantom: std::marker::PhantomData,
                });
                object.set_object_data(object_data);
                Self { object }
            }
            pub fn error(&self, error_id: u32, error_msg: impl AsRef<str>) {
                self.object.error(error_id, error_msg.as_ref());
            }
            pub fn client(&self) -> Option<hyprwire::server::ServerClient> {
                self.object.server_client()
            }
            /// Send a text message
            ///
            /// Sends a text message to the client
            ///
            /// Arguments:
            /// `message`: message
            pub fn send_send_message(&self, message: impl AsRef<str>) {
                self.object
                    .call(
                        0u32,
                        &[
                            hyprwire::core::types::CallArg::Varchar(
                                message.as_ref().as_bytes(),
                            ),
                        ],
                    );
            }
        }
    }
    pub trait TestProtocolV1Handler {
        /// Called whenever the server binds a new instance of the protocol's
        /// root object for a client.
        fn bind(&mut self, object: my_manager_v1::MyManagerV1) {
            _ = object;
        }
    }
    pub struct TestProtocolV1Impl {
        version: u32,
        handler: *mut dyn TestProtocolV1Handler,
        protocol: super::spec::TestProtocolV1ProtocolSpec,
        impls: Vec<hyprwire::implementation::server::ObjectImplementation<'static>>,
    }
    unsafe impl Send for TestProtocolV1Impl {}
    unsafe impl Sync for TestProtocolV1Impl {}
    impl TestProtocolV1Impl {
        pub fn new<
            D: TestProtocolV1Handler + hyprwire::Dispatch<my_manager_v1::MyManagerV1>
                + hyprwire::Dispatch<my_object_v1::MyObjectV1> + 'static,
        >(version: u32, handler: &mut D) -> Self {
            let handler = std::ptr::from_mut::<dyn TestProtocolV1Handler>(handler);
            Self {
                version,
                handler,
                protocol: super::spec::TestProtocolV1ProtocolSpec::default(),
                impls: vec![
                    hyprwire::implementation::server::ObjectImplementation { object_name
                    : "my_manager_v1", version, on_bind : Box::new(move | obj | { let
                    typed = my_manager_v1::MyManagerV1::new:: < D > (obj); unsafe { & mut
                    * handler } .bind(typed); }), },
                    hyprwire::implementation::server::ObjectImplementation { object_name
                    : "my_object_v1", version, on_bind : Box::new(| _obj | {}), },
                ],
            }
        }
    }
    impl hyprwire::implementation::server::ProtocolImplementations
    for TestProtocolV1Impl {
        fn protocol(&self) -> &dyn hyprwire::core::types::ProtocolSpec {
            &self.protocol
        }
        fn implementation(
            &self,
        ) -> &[hyprwire::implementation::server::ObjectImplementation<'_>] {
            &self.impls
        }
    }
    impl<D> hyprwire::implementation::server::Construct<D> for TestProtocolV1Impl
    where
        D: TestProtocolV1Handler + hyprwire::Dispatch<my_manager_v1::MyManagerV1>
            + hyprwire::Dispatch<my_object_v1::MyObjectV1> + 'static,
    {
        fn new(version: u32, handler: &mut D) -> Self {
            Self::new(version, handler)
        }
    }
}
#[allow(
    clippy::doc_markdown,
    clippy::unwrap_used,
    clippy::wildcard_imports,
    dead_code,
    unused_imports
)]
pub mod client {
    use std::{os::fd::*, rc, sync};
    pub mod my_manager_v1 {
        use super::*;
        pub type ProtocolImpl = super::TestProtocolV1Impl;
        /// manager object
        ///
        /// This object is an example manager object for the protocol
        pub struct MyManagerV1 {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for MyManagerV1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for MyManagerV1 {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for MyManagerV1 {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for MyManagerV1 {}
        impl std::hash::Hash for MyManagerV1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `my_manager_v1::MyManagerV1`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            /// Send a text message
            ///
            /// Sends a text message to the client
            SendMessage { message: String },
            /// Receive an uint array message
            ///
            /// Receives an array message to the server
            RecvMessageArrayUint { message: Vec<u32> },
        }
        struct MyManagerV1ObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for MyManagerV1ObjectData<D> {}
        unsafe impl<D> Sync for MyManagerV1ObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<my_manager_v1::MyManagerV1> + 'static,
        > hyprwire::implementation::object::ObjectData for MyManagerV1ObjectData<D> {
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
                let __proxy = my_manager_v1::MyManagerV1 {
                    object: unsafe { sync::Arc::from_raw(self.object) },
                };
                match __method {
                    0u32 => {
                        let mut __needle: usize = 0;
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
                                my_manager_v1::Event::SendMessage {
                                    message,
                                },
                            );
                    }
                    1u32 => {
                        let mut __needle: usize = 0;
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut message = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            message
                                .push(
                                    u32::from_le_bytes(
                                        __data[__needle..__needle + 4].try_into().unwrap(),
                                    ),
                                );
                            __needle += 4;
                        }
                        __dispatch
                            .event(
                                &__proxy,
                                my_manager_v1::Event::RecvMessageArrayUint {
                                    message,
                                },
                            );
                    }
                    _ => {}
                }
            }
        }
        impl hyprwire::Object for MyManagerV1 {
            type Event<'a> = Event;
            type ProtocolImpl = super::TestProtocolV1Impl;
            const NAME: &str = "my_manager_v1";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self::new::<D>(object)
            }
        }
        impl MyManagerV1 {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(MyManagerV1ObjectData::<
                    D,
                > {
                    object: sync::Arc::as_ptr(&object),
                    _phantom: std::marker::PhantomData,
                });
                object.set_object_data(object_data);
                Self { object }
            }
            /// Send a text message
            ///
            /// Sends a text message to the server
            ///
            /// Arguments:
            /// `message`: message
            pub fn send_send_message(&self, message: impl AsRef<str>) {
                self.object
                    .call(
                        0u32,
                        &[
                            hyprwire::core::types::CallArg::Varchar(
                                message.as_ref().as_bytes(),
                            ),
                        ],
                    );
            }
            /// Send a text message
            ///
            /// Sends a fd message to the server
            ///
            /// Arguments:
            /// `message`: message
            pub fn send_send_message_fd(&self, message: impl AsFd) {
                self.object
                    .call(
                        1u32,
                        &[
                            hyprwire::core::types::CallArg::Fd(
                                message.as_fd().as_raw_fd(),
                            ),
                        ],
                    );
            }
            /// Send a text message
            ///
            /// Sends a fd message to the server
            ///
            /// Arguments:
            /// `message`: message
            pub fn send_send_message_array_fd<F: AsFd>(&self, message: &[F]) {
                let message_raw_fds: Vec<i32> = message
                    .iter()
                    .map(|f| f.as_fd().as_raw_fd())
                    .collect();
                self.object
                    .call(
                        2u32,
                        &[hyprwire::core::types::CallArg::FdArray(&message_raw_fds)],
                    );
            }
            /// Send a text array message
            ///
            /// Sends an array message to the server
            ///
            /// Arguments:
            /// `message`: message
            pub fn send_send_message_array<S: AsRef<str>>(&self, message: &[S]) {
                self.object
                    .call(
                        3u32,
                        &[
                            hyprwire::core::types::CallArg::VarcharArray(
                                &message
                                    .iter()
                                    .map(|s| s.as_ref().as_bytes())
                                    .collect::<Vec<_>>(),
                            ),
                        ],
                    );
            }
            /// Send an uint array message
            ///
            /// Sends an array message to the server
            ///
            /// Arguments:
            /// `message`: message
            pub fn send_send_message_array_uint(&self, message: &[u32]) {
                self.object
                    .call(4u32, &[hyprwire::core::types::CallArg::UintArray(message)]);
            }
            /// Make a test object
            ///
            /// Makes a test object
            ///
            /// Returns a new `my_object_v1` object.
            pub fn send_make_object<
                D: hyprwire::Dispatch<super::my_object_v1::MyObjectV1> + 'static,
            >(&self) -> Option<super::my_object_v1::MyObjectV1> {
                let seq = self.object.call(5u32, &[]);
                let obj = self
                    .object
                    .client_sock()
                    .and_then(|sock| sock.object_for_seq(seq));
                Some(
                    <super::my_object_v1::MyObjectV1 as hyprwire::Object>::from_object::<
                        D,
                    >(obj?),
                )
            }
        }
    }
    pub mod my_object_v1 {
        use super::*;
        pub type ProtocolImpl = super::TestProtocolV1Impl;
        /// test object
        ///
        /// This object is an example object for the protocol
        pub struct MyObjectV1 {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for MyObjectV1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for MyObjectV1 {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for MyObjectV1 {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for MyObjectV1 {}
        impl std::hash::Hash for MyObjectV1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `my_object_v1::MyObjectV1`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            /// Send a text message
            ///
            /// Sends a text message to the client
            SendMessage { message: String },
        }
        struct MyObjectV1ObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for MyObjectV1ObjectData<D> {}
        unsafe impl<D> Sync for MyObjectV1ObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<my_object_v1::MyObjectV1> + 'static,
        > hyprwire::implementation::object::ObjectData for MyObjectV1ObjectData<D> {
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
                let __proxy = my_object_v1::MyObjectV1 {
                    object: unsafe { sync::Arc::from_raw(self.object) },
                };
                if __method == 0u32 {
                    let mut __needle: usize = 0;
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
                            my_object_v1::Event::SendMessage {
                                message,
                            },
                        );
                }
            }
        }
        impl hyprwire::Object for MyObjectV1 {
            type Event<'a> = Event;
            type ProtocolImpl = super::TestProtocolV1Impl;
            const NAME: &str = "my_object_v1";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self::new::<D>(object)
            }
        }
        impl MyObjectV1 {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(MyObjectV1ObjectData::<
                    D,
                > {
                    object: sync::Arc::as_ptr(&object),
                    _phantom: std::marker::PhantomData,
                });
                object.set_object_data(object_data);
                Self { object }
            }
            /// Send a text message
            ///
            /// Sends a text message to the server
            ///
            /// Arguments:
            /// `message`: message
            pub fn send_send_message(&self, message: impl AsRef<str>) {
                self.object
                    .call(
                        0u32,
                        &[
                            hyprwire::core::types::CallArg::Varchar(
                                message.as_ref().as_bytes(),
                            ),
                        ],
                    );
            }
            /// Send an enum message
            ///
            /// Sends an enum message to the server
            ///
            /// Arguments:
            /// `message`: message
            pub fn send_send_enum(&self, message: super::super::spec::MyEnum) {
                self.object
                    .call(1u32, &[hyprwire::core::types::CallArg::Uint(message as u32)]);
            }
            /// Make a test object
            ///
            /// Makes a test object from a test object
            ///
            /// Returns a new `my_object_v1` object.
            pub fn send_make_object<
                D: hyprwire::Dispatch<super::my_object_v1::MyObjectV1> + 'static,
            >(&self) -> Option<super::my_object_v1::MyObjectV1> {
                let seq = self.object.call(3u32, &[]);
                let obj = self
                    .object
                    .client_sock()
                    .and_then(|sock| sock.object_for_seq(seq));
                Some(
                    <super::my_object_v1::MyObjectV1 as hyprwire::Object>::from_object::<
                        D,
                    >(obj?),
                )
            }
        }
    }
    #[derive(Default, Clone)]
    pub struct TestProtocolV1Impl {
        protocol: super::spec::TestProtocolV1ProtocolSpec,
    }
    impl hyprwire::implementation::client::ProtocolImplementations
    for TestProtocolV1Impl {
        fn new() -> Self {
            Self::default()
        }
        fn protocol_spec() -> Box<dyn hyprwire::core::types::ProtocolSpec> {
            Box::new(super::spec::TestProtocolV1ProtocolSpec::default())
        }
        fn spec_name() -> &'static str {
            "test_protocol_v1"
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
    pub enum MyEnum {
        Hello = 0u32,
        World = 4u32,
    }
    #[non_exhaustive]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u32)]
    pub enum MyErrorEnum {
        OhNo = 0u32,
        ErrorImportant = 1u32,
    }
    pub struct MyManagerV1Spec {
        c2s_methods: &'static [hyprwire::core::types::Method],
        s2c_methods: &'static [hyprwire::core::types::Method],
    }
    static MY_MANAGER_V1: std::sync::LazyLock<
        std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>,
    > = std::sync::LazyLock::new(|| std::sync::Arc::new(MyManagerV1Spec {
        c2s_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[hyprwire::core::types::MessageMagic::TypeVarchar as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 1u32,
                params: &[hyprwire::core::types::MessageMagic::TypeFd as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 2u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeFd as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 3u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 4u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 5u32,
                params: &[],
                returns_type: "my_object_v1",
                since: 0,
                destructor: false,
            },
        ],
        s2c_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[hyprwire::core::types::MessageMagic::TypeVarchar as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 1u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
        ],
    }));
    impl hyprwire::core::types::ProtocolObjectSpec for MyManagerV1Spec {
        fn object_name(&self) -> &'static str {
            "my_manager_v1"
        }
        fn c2s(&self) -> &[hyprwire::core::types::Method] {
            self.c2s_methods
        }
        fn s2c(&self) -> &[hyprwire::core::types::Method] {
            self.s2c_methods
        }
    }
    pub struct MyObjectV1Spec {
        c2s_methods: &'static [hyprwire::core::types::Method],
        s2c_methods: &'static [hyprwire::core::types::Method],
    }
    static MY_OBJECT_V1: std::sync::LazyLock<
        std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>,
    > = std::sync::LazyLock::new(|| std::sync::Arc::new(MyObjectV1Spec {
        c2s_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[hyprwire::core::types::MessageMagic::TypeVarchar as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 1u32,
                params: &[hyprwire::core::types::MessageMagic::TypeUint as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 2u32,
                params: &[],
                returns_type: "",
                since: 0,
                destructor: true,
            },
            hyprwire::core::types::Method {
                idx: 3u32,
                params: &[],
                returns_type: "my_object_v1",
                since: 0,
                destructor: false,
            },
        ],
        s2c_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[hyprwire::core::types::MessageMagic::TypeVarchar as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
        ],
    }));
    impl hyprwire::core::types::ProtocolObjectSpec for MyObjectV1Spec {
        fn object_name(&self) -> &'static str {
            "my_object_v1"
        }
        fn c2s(&self) -> &[hyprwire::core::types::Method] {
            self.c2s_methods
        }
        fn s2c(&self) -> &[hyprwire::core::types::Method] {
            self.s2c_methods
        }
    }
    #[derive(Clone)]
    pub struct TestProtocolV1ProtocolSpec {
        objects: [std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>; 2usize],
    }
    impl Default for TestProtocolV1ProtocolSpec {
        fn default() -> Self {
            Self {
                objects: [
                    std::sync::Arc::clone(&MY_MANAGER_V1),
                    std::sync::Arc::clone(&MY_OBJECT_V1),
                ],
            }
        }
    }
    impl hyprwire::core::types::ProtocolSpec for TestProtocolV1ProtocolSpec {
        fn spec_name(&self) -> &'static str {
            "test_protocol_v1"
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
