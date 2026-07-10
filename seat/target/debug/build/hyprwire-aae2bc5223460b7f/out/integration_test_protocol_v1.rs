// Generated with hyprwire-scanner 0.4.3. Made with pure malice and hatred by r0chd.
// integration_test_protocol_v1
/*
 This protocol's author copyright notice is:

    Integration test protocol for full wire coverage.

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
    pub mod integration_manager_v1 {
        use super::*;
        /// integration test manager
        ///
        /// Root object used to test full protocol capabilities.
        pub struct IntegrationManagerV1 {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for IntegrationManagerV1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for IntegrationManagerV1 {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for IntegrationManagerV1 {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for IntegrationManagerV1 {}
        impl std::hash::Hash for IntegrationManagerV1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `integration_manager_v1::IntegrationManagerV1`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            SendMessage { message: String },
            SendUint { value: u32 },
            SendInt { value: i32 },
            SendFloat { value: f32 },
            SendFd { value: OwnedFd },
            SendArrayUint { values: Vec<u32> },
            SendArrayString { values: Vec<String> },
            SendArrayFd { values: Vec<OwnedFd> },
            SendStart { cmd: Vec<String>, env: Vec<String> },
            SendMixed { a: u32, b: String, c: OwnedFd, d: Vec<u32> },
            SendEnum { value: super::super::spec::TestEnum },
            MakeObject {
                integration_object_v1: super::integration_object_v1::IntegrationObjectV1,
            },
            Shutdown,
        }
        struct IntegrationManagerV1ObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for IntegrationManagerV1ObjectData<D> {}
        unsafe impl<D> Sync for IntegrationManagerV1ObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<integration_manager_v1::IntegrationManagerV1>
                + hyprwire::Dispatch<super::integration_object_v1::IntegrationObjectV1>
                + 'static,
        > hyprwire::implementation::object::ObjectData
        for IntegrationManagerV1ObjectData<D> {
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
                let __proxy = integration_manager_v1::IntegrationManagerV1 {
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
                                integration_manager_v1::Event::SendMessage {
                                    message,
                                },
                            );
                    }
                    1u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let value = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::SendUint {
                                    value,
                                },
                            );
                    }
                    2u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let value = i32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::SendInt {
                                    value,
                                },
                            );
                    }
                    3u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let value = f32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::SendFloat {
                                    value,
                                },
                            );
                    }
                    4u32 => {
                        let mut __needle: usize = 0;
                        let mut __fds_iter = __fds.iter();
                        __needle += 1;
                        let value = unsafe {
                            OwnedFd::from_raw_fd(*__fds_iter.next().unwrap())
                        };
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::SendFd {
                                    value,
                                },
                            );
                    }
                    5u32 => {
                        let mut __needle: usize = 0;
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut values = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            values
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
                                integration_manager_v1::Event::SendArrayUint {
                                    values,
                                },
                            );
                    }
                    6u32 => {
                        let mut __needle: usize = 0;
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut values = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            let (__slen, __svl) = hyprwire::core::message::parse_var_int(
                                __data,
                                __needle,
                            );
                            __needle += __svl;
                            values
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
                                integration_manager_v1::Event::SendArrayString {
                                    values,
                                },
                            );
                    }
                    7u32 => {
                        let mut __needle: usize = 0;
                        let mut __fds_iter = __fds.iter();
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut values = Vec::with_capacity(__count);
                        for &__fd in __fds_iter.by_ref().take(__count) {
                            values.push(unsafe { OwnedFd::from_raw_fd(__fd) });
                        }
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::SendArrayFd {
                                    values,
                                },
                            );
                    }
                    8u32 => {
                        let mut __needle: usize = 0;
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut cmd = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            let (__slen, __svl) = hyprwire::core::message::parse_var_int(
                                __data,
                                __needle,
                            );
                            __needle += __svl;
                            cmd.push(
                                String::from_utf8_lossy(
                                        &__data[__needle..__needle + __slen],
                                    )
                                    .into_owned(),
                            );
                            __needle += __slen;
                        }
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut env = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            let (__slen, __svl) = hyprwire::core::message::parse_var_int(
                                __data,
                                __needle,
                            );
                            __needle += __svl;
                            env.push(
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
                                integration_manager_v1::Event::SendStart {
                                    cmd,
                                    env,
                                },
                            );
                    }
                    9u32 => {
                        let mut __needle: usize = 0;
                        let mut __fds_iter = __fds.iter();
                        __needle += 1;
                        let a = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __needle += 1;
                        let (__len, __vl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __vl;
                        let b = String::from_utf8_lossy(
                                &__data[__needle..__needle + __len],
                            )
                            .into_owned();
                        __needle += __len;
                        __needle += 1;
                        let c = unsafe {
                            OwnedFd::from_raw_fd(*__fds_iter.next().unwrap())
                        };
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut d = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            d.push(
                                u32::from_le_bytes(
                                    __data[__needle..__needle + 4].try_into().unwrap(),
                                ),
                            );
                            __needle += 4;
                        }
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::SendMixed {
                                    a,
                                    b,
                                    c,
                                    d,
                                },
                            );
                    }
                    10u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let value: super::super::spec::TestEnum = unsafe {
                            std::mem::transmute::<
                                u32,
                                super::super::spec::TestEnum,
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
                                integration_manager_v1::Event::SendEnum {
                                    value,
                                },
                            );
                    }
                    11u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let __seq = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        let Some(__raw_obj) = __proxy
                            .object
                            .create_object("integration_object_v1", __seq) else {
                            return;
                        };
                        let integration_object_v1 = super::integration_object_v1::IntegrationObjectV1::new::<
                            D,
                        >(__raw_obj);
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::MakeObject {
                                    integration_object_v1,
                                },
                            );
                    }
                    12u32 => {
                        __dispatch
                            .event(&__proxy, integration_manager_v1::Event::Shutdown);
                    }
                    _ => {}
                }
            }
        }
        impl hyprwire::Object for IntegrationManagerV1 {
            type Event<'a> = Event;
            type ProtocolImpl = ();
            const NAME: &str = "integration_manager_v1";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self { object }
            }
        }
        impl IntegrationManagerV1 {
            pub fn new<
                D: hyprwire::Dispatch<Self>
                    + hyprwire::Dispatch<
                        super::integration_object_v1::IntegrationObjectV1,
                    > + 'static,
            >(object: sync::Arc<dyn hyprwire::implementation::object::Object>) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(IntegrationManagerV1ObjectData::<
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
            /// Arguments:
            /// `message`.
            pub fn send_recv_message(&self, message: impl AsRef<str>) {
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
            /// Arguments:
            /// `value`.
            pub fn send_recv_uint(&self, value: u32) {
                self.object.call(1u32, &[hyprwire::core::types::CallArg::Uint(value)]);
            }
            /// Arguments:
            /// `value`.
            pub fn send_recv_int(&self, value: i32) {
                self.object.call(2u32, &[hyprwire::core::types::CallArg::Int(value)]);
            }
            /// Arguments:
            /// `value`.
            pub fn send_recv_float(&self, value: f32) {
                self.object.call(3u32, &[hyprwire::core::types::CallArg::F32(value)]);
            }
            /// Arguments:
            /// `value`.
            pub fn send_recv_fd(&self, value: impl AsFd) {
                self.object
                    .call(
                        4u32,
                        &[hyprwire::core::types::CallArg::Fd(value.as_fd().as_raw_fd())],
                    );
            }
            /// Arguments:
            /// `values`.
            pub fn send_recv_array_uint(&self, values: &[u32]) {
                self.object
                    .call(5u32, &[hyprwire::core::types::CallArg::UintArray(values)]);
            }
            /// Arguments:
            /// `values`.
            pub fn send_recv_array_string<S: AsRef<str>>(&self, values: &[S]) {
                self.object
                    .call(
                        6u32,
                        &[
                            hyprwire::core::types::CallArg::VarcharArray(
                                &values
                                    .iter()
                                    .map(|s| s.as_ref().as_bytes())
                                    .collect::<Vec<_>>(),
                            ),
                        ],
                    );
            }
            /// Arguments:
            /// `values`.
            pub fn send_recv_array_fd<F: AsFd>(&self, values: &[F]) {
                let values_raw_fds: Vec<i32> = values
                    .iter()
                    .map(|f| f.as_fd().as_raw_fd())
                    .collect();
                self.object
                    .call(
                        7u32,
                        &[hyprwire::core::types::CallArg::FdArray(&values_raw_fds)],
                    );
            }
            /// Arguments:
            /// `cmd`.
            /// `env`.
            pub fn send_recv_start<S: AsRef<str>>(&self, cmd: &[S], env: &[S]) {
                self.object
                    .call(
                        8u32,
                        &[
                            hyprwire::core::types::CallArg::VarcharArray(
                                &cmd
                                    .iter()
                                    .map(|s| s.as_ref().as_bytes())
                                    .collect::<Vec<_>>(),
                            ),
                            hyprwire::core::types::CallArg::VarcharArray(
                                &env
                                    .iter()
                                    .map(|s| s.as_ref().as_bytes())
                                    .collect::<Vec<_>>(),
                            ),
                        ],
                    );
            }
            /// Arguments:
            /// `a`.
            /// `b`.
            /// `c`.
            /// `d`.
            pub fn send_recv_mixed(
                &self,
                a: u32,
                b: impl AsRef<str>,
                c: impl AsFd,
                d: &[u32],
            ) {
                self.object
                    .call(
                        9u32,
                        &[
                            hyprwire::core::types::CallArg::Uint(a),
                            hyprwire::core::types::CallArg::Varchar(
                                b.as_ref().as_bytes(),
                            ),
                            hyprwire::core::types::CallArg::Fd(c.as_fd().as_raw_fd()),
                            hyprwire::core::types::CallArg::UintArray(d),
                        ],
                    );
            }
            /// Arguments:
            /// `value`.
            pub fn send_recv_enum(&self, value: super::super::spec::TestEnum) {
                self.object
                    .call(10u32, &[hyprwire::core::types::CallArg::Uint(value as u32)]);
            }
            /// Arguments:
            /// `code`.
            /// `message`.
            pub fn send_report_error(
                &self,
                code: super::super::spec::TestErrorEnum,
                message: impl AsRef<str>,
            ) {
                self.object
                    .call(
                        11u32,
                        &[
                            hyprwire::core::types::CallArg::Uint(code as u32),
                            hyprwire::core::types::CallArg::Varchar(
                                message.as_ref().as_bytes(),
                            ),
                        ],
                    );
            }
        }
    }
    pub mod integration_object_v1 {
        use super::*;
        /// child test object
        ///
        /// Object used for lifecycle and nested dispatch testing.
        pub struct IntegrationObjectV1 {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for IntegrationObjectV1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for IntegrationObjectV1 {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for IntegrationObjectV1 {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for IntegrationObjectV1 {}
        impl std::hash::Hash for IntegrationObjectV1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `integration_object_v1::IntegrationObjectV1`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            Ping { value: u32 },
            MakeObject {
                integration_object_v1: super::integration_object_v1::IntegrationObjectV1,
            },
            SendEnum { value: super::super::spec::TestEnum },
            ComplexCall { e: super::super::spec::TestEnum, msg: String, nums: Vec<u32> },
        }
        struct IntegrationObjectV1ObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for IntegrationObjectV1ObjectData<D> {}
        unsafe impl<D> Sync for IntegrationObjectV1ObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<integration_object_v1::IntegrationObjectV1> + 'static,
        > hyprwire::implementation::object::ObjectData
        for IntegrationObjectV1ObjectData<D> {
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
                let __proxy = integration_object_v1::IntegrationObjectV1 {
                    object: unsafe { sync::Arc::from_raw(self.object) },
                };
                match __method {
                    0u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let value = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __dispatch
                            .event(
                                &__proxy,
                                integration_object_v1::Event::Ping {
                                    value,
                                },
                            );
                    }
                    1u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let __seq = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        let Some(__raw_obj) = __proxy
                            .object
                            .create_object("integration_object_v1", __seq) else {
                            return;
                        };
                        let integration_object_v1 = super::integration_object_v1::IntegrationObjectV1::new::<
                            D,
                        >(__raw_obj);
                        __dispatch
                            .event(
                                &__proxy,
                                integration_object_v1::Event::MakeObject {
                                    integration_object_v1,
                                },
                            );
                    }
                    2u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let value: super::super::spec::TestEnum = unsafe {
                            std::mem::transmute::<
                                u32,
                                super::super::spec::TestEnum,
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
                                integration_object_v1::Event::SendEnum {
                                    value,
                                },
                            );
                    }
                    3u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let e: super::super::spec::TestEnum = unsafe {
                            std::mem::transmute::<
                                u32,
                                super::super::spec::TestEnum,
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
                        let msg = String::from_utf8_lossy(
                                &__data[__needle..__needle + __len],
                            )
                            .into_owned();
                        __needle += __len;
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut nums = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            nums.push(
                                u32::from_le_bytes(
                                    __data[__needle..__needle + 4].try_into().unwrap(),
                                ),
                            );
                            __needle += 4;
                        }
                        __dispatch
                            .event(
                                &__proxy,
                                integration_object_v1::Event::ComplexCall {
                                    e,
                                    msg,
                                    nums,
                                },
                            );
                    }
                    _ => {}
                }
            }
        }
        impl hyprwire::Object for IntegrationObjectV1 {
            type Event<'a> = Event;
            type ProtocolImpl = ();
            const NAME: &str = "integration_object_v1";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self { object }
            }
        }
        impl IntegrationObjectV1 {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(IntegrationObjectV1ObjectData::<
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
            /// Arguments:
            /// `value`.
            pub fn send_pong(&self, value: u32) {
                self.object.call(0u32, &[hyprwire::core::types::CallArg::Uint(value)]);
            }
        }
    }
    pub trait IntegrationTestProtocolV1Handler {
        /// Called whenever the server binds a new instance of the protocol's
        /// root object for a client.
        fn bind(&mut self, object: integration_manager_v1::IntegrationManagerV1) {
            _ = object;
        }
    }
    pub struct IntegrationTestProtocolV1Impl {
        version: u32,
        handler: *mut dyn IntegrationTestProtocolV1Handler,
        protocol: super::spec::IntegrationTestProtocolV1ProtocolSpec,
        impls: Vec<hyprwire::implementation::server::ObjectImplementation<'static>>,
    }
    unsafe impl Send for IntegrationTestProtocolV1Impl {}
    unsafe impl Sync for IntegrationTestProtocolV1Impl {}
    impl IntegrationTestProtocolV1Impl {
        pub fn new<
            D: IntegrationTestProtocolV1Handler
                + hyprwire::Dispatch<integration_manager_v1::IntegrationManagerV1>
                + hyprwire::Dispatch<integration_object_v1::IntegrationObjectV1>
                + 'static,
        >(version: u32, handler: &mut D) -> Self {
            let handler = std::ptr::from_mut::<
                dyn IntegrationTestProtocolV1Handler,
            >(handler);
            Self {
                version,
                handler,
                protocol: super::spec::IntegrationTestProtocolV1ProtocolSpec::default(),
                impls: vec![
                    hyprwire::implementation::server::ObjectImplementation { object_name
                    : "integration_manager_v1", version, on_bind : Box::new(move | obj |
                    { let typed = integration_manager_v1::IntegrationManagerV1::new:: < D
                    > (obj); unsafe { & mut * handler } .bind(typed); }), },
                    hyprwire::implementation::server::ObjectImplementation { object_name
                    : "integration_object_v1", version, on_bind : Box::new(| _obj | {}),
                    },
                ],
            }
        }
    }
    impl hyprwire::implementation::server::ProtocolImplementations
    for IntegrationTestProtocolV1Impl {
        fn protocol(&self) -> &dyn hyprwire::core::types::ProtocolSpec {
            &self.protocol
        }
        fn implementation(
            &self,
        ) -> &[hyprwire::implementation::server::ObjectImplementation<'_>] {
            &self.impls
        }
    }
    impl<D> hyprwire::implementation::server::Construct<D>
    for IntegrationTestProtocolV1Impl
    where
        D: IntegrationTestProtocolV1Handler
            + hyprwire::Dispatch<integration_manager_v1::IntegrationManagerV1>
            + hyprwire::Dispatch<integration_object_v1::IntegrationObjectV1> + 'static,
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
    pub mod integration_manager_v1 {
        use super::*;
        pub type ProtocolImpl = super::IntegrationTestProtocolV1Impl;
        /// integration test manager
        ///
        /// Root object used to test full protocol capabilities.
        pub struct IntegrationManagerV1 {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for IntegrationManagerV1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for IntegrationManagerV1 {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for IntegrationManagerV1 {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for IntegrationManagerV1 {}
        impl std::hash::Hash for IntegrationManagerV1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `integration_manager_v1::IntegrationManagerV1`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            RecvMessage { message: String },
            RecvUint { value: u32 },
            RecvInt { value: i32 },
            RecvFloat { value: f32 },
            RecvFd { value: OwnedFd },
            RecvArrayUint { values: Vec<u32> },
            RecvArrayString { values: Vec<String> },
            RecvArrayFd { values: Vec<OwnedFd> },
            RecvStart { cmd: Vec<String>, env: Vec<String> },
            RecvMixed { a: u32, b: String, c: OwnedFd, d: Vec<u32> },
            RecvEnum { value: super::super::spec::TestEnum },
            ReportError { code: super::super::spec::TestErrorEnum, message: String },
        }
        struct IntegrationManagerV1ObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for IntegrationManagerV1ObjectData<D> {}
        unsafe impl<D> Sync for IntegrationManagerV1ObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<integration_manager_v1::IntegrationManagerV1> + 'static,
        > hyprwire::implementation::object::ObjectData
        for IntegrationManagerV1ObjectData<D> {
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
                let __proxy = integration_manager_v1::IntegrationManagerV1 {
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
                                integration_manager_v1::Event::RecvMessage {
                                    message,
                                },
                            );
                    }
                    1u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let value = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::RecvUint {
                                    value,
                                },
                            );
                    }
                    2u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let value = i32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::RecvInt {
                                    value,
                                },
                            );
                    }
                    3u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let value = f32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::RecvFloat {
                                    value,
                                },
                            );
                    }
                    4u32 => {
                        let mut __needle: usize = 0;
                        let mut __fds_iter = __fds.iter();
                        __needle += 1;
                        let value = unsafe {
                            OwnedFd::from_raw_fd(*__fds_iter.next().unwrap())
                        };
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::RecvFd {
                                    value,
                                },
                            );
                    }
                    5u32 => {
                        let mut __needle: usize = 0;
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut values = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            values
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
                                integration_manager_v1::Event::RecvArrayUint {
                                    values,
                                },
                            );
                    }
                    6u32 => {
                        let mut __needle: usize = 0;
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut values = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            let (__slen, __svl) = hyprwire::core::message::parse_var_int(
                                __data,
                                __needle,
                            );
                            __needle += __svl;
                            values
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
                                integration_manager_v1::Event::RecvArrayString {
                                    values,
                                },
                            );
                    }
                    7u32 => {
                        let mut __needle: usize = 0;
                        let mut __fds_iter = __fds.iter();
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut values = Vec::with_capacity(__count);
                        for &__fd in __fds_iter.by_ref().take(__count) {
                            values.push(unsafe { OwnedFd::from_raw_fd(__fd) });
                        }
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::RecvArrayFd {
                                    values,
                                },
                            );
                    }
                    8u32 => {
                        let mut __needle: usize = 0;
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut cmd = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            let (__slen, __svl) = hyprwire::core::message::parse_var_int(
                                __data,
                                __needle,
                            );
                            __needle += __svl;
                            cmd.push(
                                String::from_utf8_lossy(
                                        &__data[__needle..__needle + __slen],
                                    )
                                    .into_owned(),
                            );
                            __needle += __slen;
                        }
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut env = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            let (__slen, __svl) = hyprwire::core::message::parse_var_int(
                                __data,
                                __needle,
                            );
                            __needle += __svl;
                            env.push(
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
                                integration_manager_v1::Event::RecvStart {
                                    cmd,
                                    env,
                                },
                            );
                    }
                    9u32 => {
                        let mut __needle: usize = 0;
                        let mut __fds_iter = __fds.iter();
                        __needle += 1;
                        let a = u32::from_le_bytes(
                            __data[__needle..__needle + 4].try_into().unwrap(),
                        );
                        __needle += 4;
                        __needle += 1;
                        let (__len, __vl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __vl;
                        let b = String::from_utf8_lossy(
                                &__data[__needle..__needle + __len],
                            )
                            .into_owned();
                        __needle += __len;
                        __needle += 1;
                        let c = unsafe {
                            OwnedFd::from_raw_fd(*__fds_iter.next().unwrap())
                        };
                        __needle += 2;
                        let (__count, __cl) = hyprwire::core::message::parse_var_int(
                            __data,
                            __needle,
                        );
                        __needle += __cl;
                        let mut d = Vec::with_capacity(__count);
                        for _ in 0..__count {
                            d.push(
                                u32::from_le_bytes(
                                    __data[__needle..__needle + 4].try_into().unwrap(),
                                ),
                            );
                            __needle += 4;
                        }
                        __dispatch
                            .event(
                                &__proxy,
                                integration_manager_v1::Event::RecvMixed {
                                    a,
                                    b,
                                    c,
                                    d,
                                },
                            );
                    }
                    10u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let value: super::super::spec::TestEnum = unsafe {
                            std::mem::transmute::<
                                u32,
                                super::super::spec::TestEnum,
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
                                integration_manager_v1::Event::RecvEnum {
                                    value,
                                },
                            );
                    }
                    11u32 => {
                        let mut __needle: usize = 0;
                        __needle += 1;
                        let code: super::super::spec::TestErrorEnum = unsafe {
                            std::mem::transmute::<
                                u32,
                                super::super::spec::TestErrorEnum,
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
                                integration_manager_v1::Event::ReportError {
                                    code,
                                    message,
                                },
                            );
                    }
                    _ => {}
                }
            }
        }
        impl hyprwire::Object for IntegrationManagerV1 {
            type Event<'a> = Event;
            type ProtocolImpl = super::IntegrationTestProtocolV1Impl;
            const NAME: &str = "integration_manager_v1";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self::new::<D>(object)
            }
        }
        impl IntegrationManagerV1 {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(IntegrationManagerV1ObjectData::<
                    D,
                > {
                    object: sync::Arc::as_ptr(&object),
                    _phantom: std::marker::PhantomData,
                });
                object.set_object_data(object_data);
                Self { object }
            }
            /// Arguments:
            /// `message`.
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
            /// Arguments:
            /// `value`.
            pub fn send_send_uint(&self, value: u32) {
                self.object.call(1u32, &[hyprwire::core::types::CallArg::Uint(value)]);
            }
            /// Arguments:
            /// `value`.
            pub fn send_send_int(&self, value: i32) {
                self.object.call(2u32, &[hyprwire::core::types::CallArg::Int(value)]);
            }
            /// Arguments:
            /// `value`.
            pub fn send_send_float(&self, value: f32) {
                self.object.call(3u32, &[hyprwire::core::types::CallArg::F32(value)]);
            }
            /// Arguments:
            /// `value`.
            pub fn send_send_fd(&self, value: impl AsFd) {
                self.object
                    .call(
                        4u32,
                        &[hyprwire::core::types::CallArg::Fd(value.as_fd().as_raw_fd())],
                    );
            }
            /// Arguments:
            /// `values`.
            pub fn send_send_array_uint(&self, values: &[u32]) {
                self.object
                    .call(5u32, &[hyprwire::core::types::CallArg::UintArray(values)]);
            }
            /// Arguments:
            /// `values`.
            pub fn send_send_array_string<S: AsRef<str>>(&self, values: &[S]) {
                self.object
                    .call(
                        6u32,
                        &[
                            hyprwire::core::types::CallArg::VarcharArray(
                                &values
                                    .iter()
                                    .map(|s| s.as_ref().as_bytes())
                                    .collect::<Vec<_>>(),
                            ),
                        ],
                    );
            }
            /// Arguments:
            /// `values`.
            pub fn send_send_array_fd<F: AsFd>(&self, values: &[F]) {
                let values_raw_fds: Vec<i32> = values
                    .iter()
                    .map(|f| f.as_fd().as_raw_fd())
                    .collect();
                self.object
                    .call(
                        7u32,
                        &[hyprwire::core::types::CallArg::FdArray(&values_raw_fds)],
                    );
            }
            /// Arguments:
            /// `cmd`.
            /// `env`.
            pub fn send_send_start<S: AsRef<str>>(&self, cmd: &[S], env: &[S]) {
                self.object
                    .call(
                        8u32,
                        &[
                            hyprwire::core::types::CallArg::VarcharArray(
                                &cmd
                                    .iter()
                                    .map(|s| s.as_ref().as_bytes())
                                    .collect::<Vec<_>>(),
                            ),
                            hyprwire::core::types::CallArg::VarcharArray(
                                &env
                                    .iter()
                                    .map(|s| s.as_ref().as_bytes())
                                    .collect::<Vec<_>>(),
                            ),
                        ],
                    );
            }
            /// Arguments:
            /// `a`.
            /// `b`.
            /// `c`.
            /// `d`.
            pub fn send_send_mixed(
                &self,
                a: u32,
                b: impl AsRef<str>,
                c: impl AsFd,
                d: &[u32],
            ) {
                self.object
                    .call(
                        9u32,
                        &[
                            hyprwire::core::types::CallArg::Uint(a),
                            hyprwire::core::types::CallArg::Varchar(
                                b.as_ref().as_bytes(),
                            ),
                            hyprwire::core::types::CallArg::Fd(c.as_fd().as_raw_fd()),
                            hyprwire::core::types::CallArg::UintArray(d),
                        ],
                    );
            }
            /// Arguments:
            /// `value`.
            pub fn send_send_enum(&self, value: super::super::spec::TestEnum) {
                self.object
                    .call(10u32, &[hyprwire::core::types::CallArg::Uint(value as u32)]);
            }
            /// Returns a new `integration_object_v1` object.
            pub fn send_make_object<
                D: hyprwire::Dispatch<super::integration_object_v1::IntegrationObjectV1>
                    + 'static,
            >(&self) -> Option<super::integration_object_v1::IntegrationObjectV1> {
                let seq = self.object.call(11u32, &[]);
                let obj = self
                    .object
                    .client_sock()
                    .and_then(|sock| sock.object_for_seq(seq));
                Some(
                    <super::integration_object_v1::IntegrationObjectV1 as hyprwire::Object>::from_object::<
                        D,
                    >(obj?),
                )
            }
            pub fn send_shutdown(&self) {
                self.object.call(12u32, &[]);
            }
        }
    }
    pub mod integration_object_v1 {
        use super::*;
        pub type ProtocolImpl = super::IntegrationTestProtocolV1Impl;
        /// child test object
        ///
        /// Object used for lifecycle and nested dispatch testing.
        pub struct IntegrationObjectV1 {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for IntegrationObjectV1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for IntegrationObjectV1 {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for IntegrationObjectV1 {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for IntegrationObjectV1 {}
        impl std::hash::Hash for IntegrationObjectV1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `integration_object_v1::IntegrationObjectV1`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            Pong { value: u32 },
        }
        struct IntegrationObjectV1ObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for IntegrationObjectV1ObjectData<D> {}
        unsafe impl<D> Sync for IntegrationObjectV1ObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<integration_object_v1::IntegrationObjectV1> + 'static,
        > hyprwire::implementation::object::ObjectData
        for IntegrationObjectV1ObjectData<D> {
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
                let __proxy = integration_object_v1::IntegrationObjectV1 {
                    object: unsafe { sync::Arc::from_raw(self.object) },
                };
                if __method == 0u32 {
                    let mut __needle: usize = 0;
                    __needle += 1;
                    let value = u32::from_le_bytes(
                        __data[__needle..__needle + 4].try_into().unwrap(),
                    );
                    __needle += 4;
                    __dispatch
                        .event(
                            &__proxy,
                            integration_object_v1::Event::Pong {
                                value,
                            },
                        );
                }
            }
        }
        impl hyprwire::Object for IntegrationObjectV1 {
            type Event<'a> = Event;
            type ProtocolImpl = super::IntegrationTestProtocolV1Impl;
            const NAME: &str = "integration_object_v1";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self::new::<D>(object)
            }
        }
        impl IntegrationObjectV1 {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(IntegrationObjectV1ObjectData::<
                    D,
                > {
                    object: sync::Arc::as_ptr(&object),
                    _phantom: std::marker::PhantomData,
                });
                object.set_object_data(object_data);
                Self { object }
            }
            /// Arguments:
            /// `value`.
            pub fn send_ping(&self, value: u32) {
                self.object.call(0u32, &[hyprwire::core::types::CallArg::Uint(value)]);
            }
            /// Returns a new `integration_object_v1` object.
            pub fn send_make_object<
                D: hyprwire::Dispatch<super::integration_object_v1::IntegrationObjectV1>
                    + 'static,
            >(&self) -> Option<super::integration_object_v1::IntegrationObjectV1> {
                let seq = self.object.call(1u32, &[]);
                let obj = self
                    .object
                    .client_sock()
                    .and_then(|sock| sock.object_for_seq(seq));
                Some(
                    <super::integration_object_v1::IntegrationObjectV1 as hyprwire::Object>::from_object::<
                        D,
                    >(obj?),
                )
            }
            /// Arguments:
            /// `value`.
            pub fn send_send_enum(&self, value: super::super::spec::TestEnum) {
                self.object
                    .call(2u32, &[hyprwire::core::types::CallArg::Uint(value as u32)]);
            }
            /// Arguments:
            /// `e`.
            /// `msg`.
            /// `nums`.
            pub fn send_complex_call(
                &self,
                e: super::super::spec::TestEnum,
                msg: impl AsRef<str>,
                nums: &[u32],
            ) {
                self.object
                    .call(
                        3u32,
                        &[
                            hyprwire::core::types::CallArg::Uint(e as u32),
                            hyprwire::core::types::CallArg::Varchar(
                                msg.as_ref().as_bytes(),
                            ),
                            hyprwire::core::types::CallArg::UintArray(nums),
                        ],
                    );
            }
        }
    }
    #[derive(Default, Clone)]
    pub struct IntegrationTestProtocolV1Impl {
        protocol: super::spec::IntegrationTestProtocolV1ProtocolSpec,
    }
    impl hyprwire::implementation::client::ProtocolImplementations
    for IntegrationTestProtocolV1Impl {
        fn new() -> Self {
            Self::default()
        }
        fn protocol_spec() -> Box<dyn hyprwire::core::types::ProtocolSpec> {
            Box::new(super::spec::IntegrationTestProtocolV1ProtocolSpec::default())
        }
        fn spec_name() -> &'static str {
            "integration_test_protocol_v1"
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
    pub enum TestEnum {
        Hello = 0u32,
        World = 4u32,
    }
    #[non_exhaustive]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u32)]
    pub enum TestErrorEnum {
        OhNo = 0u32,
        ImportantError = 1u32,
    }
    pub struct IntegrationManagerV1Spec {
        c2s_methods: &'static [hyprwire::core::types::Method],
        s2c_methods: &'static [hyprwire::core::types::Method],
    }
    static INTEGRATION_MANAGER_V1: std::sync::LazyLock<
        std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>,
    > = std::sync::LazyLock::new(|| std::sync::Arc::new(IntegrationManagerV1Spec {
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
                params: &[hyprwire::core::types::MessageMagic::TypeInt as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 3u32,
                params: &[hyprwire::core::types::MessageMagic::TypeF32 as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 4u32,
                params: &[hyprwire::core::types::MessageMagic::TypeFd as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 5u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 6u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 7u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeFd as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 8u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 9u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                    hyprwire::core::types::MessageMagic::TypeFd as u8,
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 10u32,
                params: &[hyprwire::core::types::MessageMagic::TypeUint as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 11u32,
                params: &[],
                returns_type: "integration_object_v1",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 12u32,
                params: &[],
                returns_type: "",
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
                params: &[hyprwire::core::types::MessageMagic::TypeUint as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 2u32,
                params: &[hyprwire::core::types::MessageMagic::TypeInt as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 3u32,
                params: &[hyprwire::core::types::MessageMagic::TypeF32 as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 4u32,
                params: &[hyprwire::core::types::MessageMagic::TypeFd as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 5u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 6u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 7u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeFd as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 8u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 9u32,
                params: &[
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                    hyprwire::core::types::MessageMagic::TypeVarchar as u8,
                    hyprwire::core::types::MessageMagic::TypeFd as u8,
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 10u32,
                params: &[hyprwire::core::types::MessageMagic::TypeUint as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 11u32,
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
    impl hyprwire::core::types::ProtocolObjectSpec for IntegrationManagerV1Spec {
        fn object_name(&self) -> &'static str {
            "integration_manager_v1"
        }
        fn c2s(&self) -> &[hyprwire::core::types::Method] {
            self.c2s_methods
        }
        fn s2c(&self) -> &[hyprwire::core::types::Method] {
            self.s2c_methods
        }
    }
    pub struct IntegrationObjectV1Spec {
        c2s_methods: &'static [hyprwire::core::types::Method],
        s2c_methods: &'static [hyprwire::core::types::Method],
    }
    static INTEGRATION_OBJECT_V1: std::sync::LazyLock<
        std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>,
    > = std::sync::LazyLock::new(|| std::sync::Arc::new(IntegrationObjectV1Spec {
        c2s_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[hyprwire::core::types::MessageMagic::TypeUint as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
            hyprwire::core::types::Method {
                idx: 1u32,
                params: &[],
                returns_type: "integration_object_v1",
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
                    hyprwire::core::types::MessageMagic::TypeArray as u8,
                    hyprwire::core::types::MessageMagic::TypeUint as u8,
                ],
                returns_type: "",
                since: 0,
                destructor: false,
            },
        ],
        s2c_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[hyprwire::core::types::MessageMagic::TypeUint as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
        ],
    }));
    impl hyprwire::core::types::ProtocolObjectSpec for IntegrationObjectV1Spec {
        fn object_name(&self) -> &'static str {
            "integration_object_v1"
        }
        fn c2s(&self) -> &[hyprwire::core::types::Method] {
            self.c2s_methods
        }
        fn s2c(&self) -> &[hyprwire::core::types::Method] {
            self.s2c_methods
        }
    }
    #[derive(Clone)]
    pub struct IntegrationTestProtocolV1ProtocolSpec {
        objects: [std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>; 2usize],
    }
    impl Default for IntegrationTestProtocolV1ProtocolSpec {
        fn default() -> Self {
            Self {
                objects: [
                    std::sync::Arc::clone(&INTEGRATION_MANAGER_V1),
                    std::sync::Arc::clone(&INTEGRATION_OBJECT_V1),
                ],
            }
        }
    }
    impl hyprwire::core::types::ProtocolSpec for IntegrationTestProtocolV1ProtocolSpec {
        fn spec_name(&self) -> &'static str {
            "integration_test_protocol_v1"
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
