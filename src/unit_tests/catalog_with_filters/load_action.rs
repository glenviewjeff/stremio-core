use crate::models::catalog_with_filters::{CatalogWithFilters, Selected};
use crate::models::common::{Loadable, ResourceLoadable};
use crate::models::ctx::Ctx;
use crate::runtime::msg::{Action, ActionLoad};
use crate::runtime::{EnvFutureExt, Runtime, RuntimeAction, RuntimeEvent, TryEnvFuture};
use crate::types::addon::ResourceResponse;
use crate::types::resource::MetaItemPreview;
use crate::unit_tests::{
    default_fetch_handler, Request, TestEnv, EVENTS, FETCH_HANDLER, REQUESTS, STATES,
};
use assert_matches::assert_matches;
use enclose::enclose;
use futures::future;
use std::any::Any;
use std::sync::{Arc, RwLock};
use stremio_derive::Model;

#[test]
fn load_action() {
    #[derive(Model, Default, Clone)]
    #[model(TestEnv)]
    struct TestModel {
        ctx: Ctx,
        discover: CatalogWithFilters<MetaItemPreview>,
    }
    fn fetch_handler(request: Request) -> TryEnvFuture<Box<dyn Any + Send>> {
        match request {
            Request { url, method, .. }
                if url == "https://v3-cinemeta.strem.io/catalog/movie/top.json"
                    && method == "GET" =>
            {
                future::ok(Box::new(ResourceResponse::Metas {
                    metas: vec![MetaItemPreview::default()],
                }) as Box<dyn Any + Send>)
                .boxed_env()
            }
            _ => default_fetch_handler(request),
        }
    }
    let _env_mutex = TestEnv::reset();
    *FETCH_HANDLER.write().unwrap() = Box::new(fetch_handler);
    let ctx = Ctx::default();
    let (discover, effects) = CatalogWithFilters::<MetaItemPreview>::new(&ctx.profile);
    let (runtime, rx) = Runtime::<TestEnv, _>::new(TestModel { ctx, discover }, effects, 1000);
    let runtime = Arc::new(RwLock::new(runtime));
    TestEnv::run_with_runtime(
        rx,
        runtime.clone(),
        enclose!((runtime) move || {
            let runtime = runtime.read().unwrap();
            runtime.dispatch(RuntimeAction {
                field: None,
                action: Action::Load(ActionLoad::CatalogWithFilters(None)),
            });
        }),
    );
    let events = EVENTS.read().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], RuntimeEvent::NewState);
    assert_eq!(events[1], RuntimeEvent::NewState);
    let states = STATES.read().unwrap();
    let states = states
        .iter()
        .map(|state| state.downcast_ref::<TestModel>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(states.len(), 3);
    assert!(states[1].discover.selectable.next_page.is_none());
    assert_matches!(&states[1].discover.selected, Some(Selected { request }) if *request == states[0].discover.selectable.types.first().unwrap().request);
    assert_matches!(
        states[1].discover.catalog.first(),
        Some(ResourceLoadable {
            content: Some(Loadable::Loading),
            ..
        })
    );
    assert!(states[2].discover.selectable.next_page.is_some());
    assert_matches!(
        states[2].discover.catalog.first(),
        Some(ResourceLoadable {
            content: Some(Loadable::Ready(..)),
            ..
        })
    );
    let requests = REQUESTS.read().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0],
        Request {
            url: "https://v3-cinemeta.strem.io/catalog/movie/top.json".to_owned(),
            method: "GET".to_owned(),
            headers: Default::default(),
            body: "null".to_owned()
        }
    )
}
