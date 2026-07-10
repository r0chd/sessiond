// Generated with hyprwire-scanner 0.4.3. Made with pure malice and hatred by r0chd.
// bench_protocol_v1
/*
 This protocol's author copyright notice is:

    Benchmark-only protocol.

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
    pub mod bench_v1 {
        use super::*;
        /// benchmark object
        ///
        /// Minimal protocol used by Criterion benches. Single root object, no extra objects.
        pub struct BenchV1 {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for BenchV1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for BenchV1 {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for BenchV1 {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for BenchV1 {}
        impl std::hash::Hash for BenchV1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `bench_v1::BenchV1`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {
            /// client->server message
            ///
            /// Sends a text message to the server.
            SendMessage { message: String },
        }
        struct BenchV1ObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for BenchV1ObjectData<D> {}
        unsafe impl<D> Sync for BenchV1ObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<bench_v1::BenchV1> + 'static,
        > hyprwire::implementation::object::ObjectData for BenchV1ObjectData<D> {
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
                let __proxy = bench_v1::BenchV1 {
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
                            bench_v1::Event::SendMessage {
                                message,
                            },
                        );
                }
            }
        }
        impl hyprwire::Object for BenchV1 {
            type Event<'a> = Event;
            type ProtocolImpl = ();
            const NAME: &str = "bench_v1";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self { object }
            }
        }
        impl BenchV1 {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(BenchV1ObjectData::<
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
        }
    }
    pub trait BenchProtocolV1Handler {
        /// Called whenever the server binds a new instance of the protocol's
        /// root object for a client.
        fn bind(&mut self, object: bench_v1::BenchV1) {
            _ = object;
        }
    }
    pub struct BenchProtocolV1Impl {
        version: u32,
        handler: *mut dyn BenchProtocolV1Handler,
        protocol: super::spec::BenchProtocolV1ProtocolSpec,
        impls: Vec<hyprwire::implementation::server::ObjectImplementation<'static>>,
    }
    unsafe impl Send for BenchProtocolV1Impl {}
    unsafe impl Sync for BenchProtocolV1Impl {}
    impl BenchProtocolV1Impl {
        pub fn new<
            D: BenchProtocolV1Handler + hyprwire::Dispatch<bench_v1::BenchV1> + 'static,
        >(version: u32, handler: &mut D) -> Self {
            let handler = std::ptr::from_mut::<dyn BenchProtocolV1Handler>(handler);
            Self {
                version,
                handler,
                protocol: super::spec::BenchProtocolV1ProtocolSpec::default(),
                impls: vec![
                    hyprwire::implementation::server::ObjectImplementation { object_name
                    : "bench_v1", version, on_bind : Box::new(move | obj | { let typed =
                    bench_v1::BenchV1::new:: < D > (obj); unsafe { & mut * handler }
                    .bind(typed); }), },
                ],
            }
        }
    }
    impl hyprwire::implementation::server::ProtocolImplementations
    for BenchProtocolV1Impl {
        fn protocol(&self) -> &dyn hyprwire::core::types::ProtocolSpec {
            &self.protocol
        }
        fn implementation(
            &self,
        ) -> &[hyprwire::implementation::server::ObjectImplementation<'_>] {
            &self.impls
        }
    }
    impl<D> hyprwire::implementation::server::Construct<D> for BenchProtocolV1Impl
    where
        D: BenchProtocolV1Handler + hyprwire::Dispatch<bench_v1::BenchV1> + 'static,
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
    pub mod bench_v1 {
        use super::*;
        pub type ProtocolImpl = super::BenchProtocolV1Impl;
        /// benchmark object
        ///
        /// Minimal protocol used by Criterion benches. Single root object, no extra objects.
        pub struct BenchV1 {
            pub(super) object: sync::Arc<dyn hyprwire::implementation::object::Object>,
        }
        impl std::fmt::Debug for BenchV1 {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Object")
                    .field("object", &sync::Arc::as_ptr(&self.object))
                    .finish()
            }
        }
        impl Clone for BenchV1 {
            fn clone(&self) -> Self {
                Self {
                    object: sync::Arc::clone(&self.object),
                }
            }
        }
        impl PartialEq for BenchV1 {
            fn eq(&self, other: &Self) -> bool {
                sync::Arc::ptr_eq(&self.object, &other.object)
            }
        }
        impl Eq for BenchV1 {}
        impl std::hash::Hash for BenchV1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                sync::Arc::as_ptr(&self.object).hash(state);
            }
        }
        ///Incoming events for `bench_v1::BenchV1`.
        #[non_exhaustive]
        #[derive(Debug)]
        pub enum Event {}
        struct BenchV1ObjectData<D> {
            object: *const dyn hyprwire::implementation::object::Object,
            _phantom: std::marker::PhantomData<D>,
        }
        unsafe impl<D> Send for BenchV1ObjectData<D> {}
        unsafe impl<D> Sync for BenchV1ObjectData<D> {}
        impl<
            D: hyprwire::Dispatch<bench_v1::BenchV1> + 'static,
        > hyprwire::implementation::object::ObjectData for BenchV1ObjectData<D> {
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
                let __proxy = bench_v1::BenchV1 {
                    object: unsafe { sync::Arc::from_raw(self.object) },
                };
            }
        }
        impl hyprwire::Object for BenchV1 {
            type Event<'a> = Event;
            type ProtocolImpl = super::BenchProtocolV1Impl;
            const NAME: &str = "bench_v1";
            fn from_object<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                Self::new::<D>(object)
            }
        }
        impl BenchV1 {
            pub fn new<D: hyprwire::Dispatch<Self> + 'static>(
                object: sync::Arc<dyn hyprwire::implementation::object::Object>,
            ) -> Self {
                let object_data: Box<dyn hyprwire::implementation::object::ObjectData> = Box::new(BenchV1ObjectData::<
                    D,
                > {
                    object: sync::Arc::as_ptr(&object),
                    _phantom: std::marker::PhantomData,
                });
                object.set_object_data(object_data);
                Self { object }
            }
            /// client->server message
            ///
            /// Sends a text message to the server.
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
    #[derive(Default, Clone)]
    pub struct BenchProtocolV1Impl {
        protocol: super::spec::BenchProtocolV1ProtocolSpec,
    }
    impl hyprwire::implementation::client::ProtocolImplementations
    for BenchProtocolV1Impl {
        fn new() -> Self {
            Self::default()
        }
        fn protocol_spec() -> Box<dyn hyprwire::core::types::ProtocolSpec> {
            Box::new(super::spec::BenchProtocolV1ProtocolSpec::default())
        }
        fn spec_name() -> &'static str {
            "bench_protocol_v1"
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
    pub struct BenchV1Spec {
        c2s_methods: &'static [hyprwire::core::types::Method],
        s2c_methods: &'static [hyprwire::core::types::Method],
    }
    static BENCH_V1: std::sync::LazyLock<
        std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>,
    > = std::sync::LazyLock::new(|| std::sync::Arc::new(BenchV1Spec {
        c2s_methods: &[
            hyprwire::core::types::Method {
                idx: 0u32,
                params: &[hyprwire::core::types::MessageMagic::TypeVarchar as u8],
                returns_type: "",
                since: 0,
                destructor: false,
            },
        ],
        s2c_methods: &[],
    }));
    impl hyprwire::core::types::ProtocolObjectSpec for BenchV1Spec {
        fn object_name(&self) -> &'static str {
            "bench_v1"
        }
        fn c2s(&self) -> &[hyprwire::core::types::Method] {
            self.c2s_methods
        }
        fn s2c(&self) -> &[hyprwire::core::types::Method] {
            self.s2c_methods
        }
    }
    #[derive(Clone)]
    pub struct BenchProtocolV1ProtocolSpec {
        objects: [std::sync::Arc<dyn hyprwire::core::types::ProtocolObjectSpec>; 1usize],
    }
    impl Default for BenchProtocolV1ProtocolSpec {
        fn default() -> Self {
            Self {
                objects: [std::sync::Arc::clone(&BENCH_V1)],
            }
        }
    }
    impl hyprwire::core::types::ProtocolSpec for BenchProtocolV1ProtocolSpec {
        fn spec_name(&self) -> &'static str {
            "bench_protocol_v1"
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
