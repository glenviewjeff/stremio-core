use crate::models::ctx::Ctx;
use crate::runtime::msg::{Action, ActionCtx};
use crate::runtime::{Effects, Env, EnvFutureExt, Runtime, RuntimeAction, TryEnvFuture};
use crate::types::addon::{Descriptor, Manifest};
use crate::types::api::{APIResult, SuccessResponse};
use crate::types::profile::{Auth, AuthKey, GDPRConsent, Profile, User};
use crate::types::True;
use crate::unit_tests::{default_fetch_handler, Request, TestEnv, FETCH_HANDLER, REQUESTS};
use futures::future;
use semver::Version;
use std::any::Any;
use stremio_derive::Model;
use url::Url;

#[test]
fn actionctx_pushaddonstoapi() {
    #[derive(Model, Default)]
    #[model(TestEnv)]
    struct TestModel {
        ctx: Ctx,
    }
    let _env_mutex = TestEnv::reset();
    let (runtime, _rx) = Runtime::<TestEnv, _>::new(
        TestModel {
            ctx: Ctx {
                profile: Profile {
                    addons: vec![Descriptor {
                        manifest: Manifest {
                            id: "id".to_owned(),
                            version: Version::new(0, 0, 1),
                            name: "name".to_owned(),
                            contact_email: None,
                            description: None,
                            logo: None,
                            background: None,
                            types: vec![],
                            resources: vec![],
                            id_prefixes: None,
                            catalogs: vec![],
                            addon_catalogs: vec![],
                            behavior_hints: Default::default(),
                        },
                        transport_url: Url::parse("https://transport_url").unwrap(),
                        flags: Default::default(),
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
        },
        Effects::none().unchanged(),
        1000,
    );
    TestEnv::run(|| {
        runtime.dispatch(RuntimeAction {
            field: None,
            action: Action::Ctx(ActionCtx::PushAddonsToAPI),
        })
    });
    assert!(
        REQUESTS.read().unwrap().is_empty(),
        "No requests have been sent"
    );
}

#[test]
fn actionctx_pushaddonstoapi_with_user() {
    #[derive(Model, Default)]
    #[model(TestEnv)]
    struct TestModel {
        ctx: Ctx,
    }
    fn fetch_handler(request: Request) -> TryEnvFuture<Box<dyn Any + Send>> {
        match request {
            Request {
                url, method, body, ..
            } if url == "https://api.strem.io/api/addonCollectionSet"
                && method == "POST"
                && body == "{\"type\":\"AddonCollectionSet\",\"authKey\":\"auth_key\",\"addons\":[{\"manifest\":{\"id\":\"id\",\"version\":\"0.0.1\",\"name\":\"name\",\"contactEmail\":null,\"description\":null,\"logo\":null,\"background\":null,\"types\":[],\"resources\":[],\"idPrefixes\":null,\"catalogs\":[],\"addonCatalogs\":[],\"behaviorHints\":{\"adult\":false,\"p2p\":false,\"configurable\":false,\"configurationRequired\":false}},\"transportUrl\":\"https://transport_url/\",\"flags\":{\"official\":false,\"protected\":false}}]}" =>
            {
                future::ok(Box::new(APIResult::Ok {
                    result: SuccessResponse { success: True {} },
                }) as Box<dyn Any + Send>).boxed_env()
            }
            _ => default_fetch_handler(request),
        }
    }
    let _env_mutex = TestEnv::reset();
    *FETCH_HANDLER.write().unwrap() = Box::new(fetch_handler);
    let (runtime, _rx) = Runtime::<TestEnv, _>::new(
        TestModel {
            ctx: Ctx {
                profile: Profile {
                    auth: Some(Auth {
                        key: AuthKey("auth_key".to_owned()),
                        user: User {
                            id: "user_id".to_owned(),
                            email: "user_email".to_owned(),
                            fb_id: None,
                            avatar: None,
                            last_modified: TestEnv::now(),
                            date_registered: TestEnv::now(),
                            gdpr_consent: GDPRConsent {
                                tos: true,
                                privacy: true,
                                marketing: true,
                            },
                        },
                    }),
                    addons: vec![Descriptor {
                        manifest: Manifest {
                            id: "id".to_owned(),
                            version: Version::new(0, 0, 1),
                            name: "name".to_owned(),
                            contact_email: None,
                            description: None,
                            logo: None,
                            background: None,
                            types: vec![],
                            resources: vec![],
                            id_prefixes: None,
                            catalogs: vec![],
                            addon_catalogs: vec![],
                            behavior_hints: Default::default(),
                        },
                        transport_url: Url::parse("https://transport_url").unwrap(),
                        flags: Default::default(),
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
        },
        Effects::none().unchanged(),
        1000,
    );
    TestEnv::run(|| {
        runtime.dispatch(RuntimeAction {
            field: None,
            action: Action::Ctx(ActionCtx::PushAddonsToAPI),
        })
    });
    assert_eq!(
        REQUESTS.read().unwrap().len(),
        1,
        "One request has been sent"
    );
    assert_eq!(
        REQUESTS.read().unwrap().get(0).unwrap().to_owned(),
        Request {
            url: "https://api.strem.io/api/addonCollectionSet".to_owned(),
            method: "POST".to_owned(),
            body: "{\"type\":\"AddonCollectionSet\",\"authKey\":\"auth_key\",\"addons\":[{\"manifest\":{\"id\":\"id\",\"version\":\"0.0.1\",\"name\":\"name\",\"contactEmail\":null,\"description\":null,\"logo\":null,\"background\":null,\"types\":[],\"resources\":[],\"idPrefixes\":null,\"catalogs\":[],\"addonCatalogs\":[],\"behaviorHints\":{\"adult\":false,\"p2p\":false,\"configurable\":false,\"configurationRequired\":false}},\"transportUrl\":\"https://transport_url/\",\"flags\":{\"official\":false,\"protected\":false}}]}"
                .to_owned(),
            ..Default::default()
        },
        "addonCollectionSet request has been sent"
    );
}
