use crate::constants::{META_RESOURCE_NAME, STREAM_RESOURCE_NAME};
use crate::models::common::{
    eq_update, resources_update, resources_update_with_vector_content, Loadable, ResourceLoadable,
    ResourcesAction, ResourcesRequestRange,
};
use crate::models::ctx::Ctx;
use crate::runtime::msg::{Action, ActionLoad, ActionMetaDetails, Internal, Msg};
use crate::runtime::{Effects, Env, UpdateWithCtx};
use crate::types::addon::{AggrRequest, ResourcePath};
use crate::types::library::{LibraryBucket, LibraryItem, LibraryItemState};
use crate::types::resource::{MetaItem, Stream};
use serde::{Deserialize, Serialize};
use stremio_watched_bitfield::WatchedBitField;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selected {
    pub meta_path: ResourcePath,
    pub stream_path: Option<ResourcePath>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaDetails {
    pub selected: Option<Selected>,
    pub meta_items: Vec<ResourceLoadable<MetaItem>>,
    pub streams: Vec<ResourceLoadable<Vec<Stream>>>,
    pub library_item: Option<LibraryItem>,
    #[serde(skip_serializing)]
    pub watched: Vec<Option<WatchedBitField>>,
}

impl<E: Env + 'static> UpdateWithCtx<E> for MetaDetails {
    fn update(&mut self, msg: &Msg, ctx: &Ctx) -> Effects {
        match msg {
            Msg::Action(Action::Load(ActionLoad::MetaDetails(selected))) => {
                let selected_effects = eq_update(&mut self.selected, Some(selected.to_owned()));
                let meta_items_effects = resources_update::<E, _>(
                    &mut self.meta_items,
                    ResourcesAction::ResourcesRequested {
                        request: &AggrRequest::AllOfResource(selected.meta_path.to_owned()),
                        addons: &ctx.profile.addons,
                        range: &Some(ResourcesRequestRange::All),
                    },
                );
                let streams_effects = match &selected.stream_path {
                    Some(stream_path) => {
                        if let Some(streams) =
                            streams_from_meta_items(&self.meta_items, &stream_path.id)
                        {
                            eq_update(&mut self.streams, vec![streams])
                        } else {
                            resources_update_with_vector_content::<E, _>(
                                &mut self.streams,
                                ResourcesAction::ResourcesRequested {
                                    request: &AggrRequest::AllOfResource(stream_path.to_owned()),
                                    addons: &ctx.profile.addons,
                                    range: &Some(ResourcesRequestRange::All),
                                },
                            )
                        }
                    }
                    None => eq_update(&mut self.streams, vec![]),
                };
                let library_item_effects =
                    library_item_update(&mut self.library_item, &self.selected, &ctx.library);
                let watched_effects =
                    watched_update::<E>(&mut self.watched, &self.meta_items, &self.library_item);
                selected_effects
                    .join(meta_items_effects)
                    .join(streams_effects)
                    .join(library_item_effects)
                    .join(watched_effects)
            }
            Msg::Action(Action::Unload) => {
                let selected_effects = eq_update(&mut self.selected, None);
                let meta_items_effects = eq_update(&mut self.meta_items, vec![]);
                let streams_effects = eq_update(&mut self.streams, vec![]);
                let library_item_effects = eq_update(&mut self.library_item, None);
                let watched_effects = eq_update(&mut self.watched, vec![]);
                selected_effects
                    .join(meta_items_effects)
                    .join(streams_effects)
                    .join(library_item_effects)
                    .join(watched_effects)
            }
            Msg::Action(Action::MetaDetails(ActionMetaDetails::MarkAsWatched(
                video_id,
                is_watched,
            ))) => {
                let watched = self
                    .meta_items
                    .iter()
                    .zip(self.watched.iter())
                    .find(|(meta_item, _)| matches!(&meta_item.content, Some(Loadable::Ready(_))))
                    .map(|(_, watched)| watched.as_ref())
                    .flatten();
                match (&self.library_item, watched) {
                    (Some(library_item), Some(watched)) => {
                        let mut library_item = library_item.to_owned();
                        let mut watched = watched.to_owned();
                        watched.set_video(video_id, *is_watched);
                        library_item.state.watched = Some(watched.to_string());
                        Effects::msg(Msg::Internal(Internal::UpdateLibraryItem(library_item)))
                            .unchanged()
                    }
                    _ => Effects::none().unchanged(),
                }
            }
            Msg::Internal(Internal::ResourceRequestResult(request, result))
                if request.path.resource == META_RESOURCE_NAME =>
            {
                let meta_items_effects = resources_update::<E, _>(
                    &mut self.meta_items,
                    ResourcesAction::ResourceRequestResult {
                        request,
                        result,
                        limit: &None,
                    },
                );
                let streams_effects = match &self.selected {
                    Some(Selected {
                        stream_path: Some(stream_path),
                        ..
                    }) => {
                        if let Some(streams) =
                            streams_from_meta_items(&self.meta_items, &stream_path.id)
                        {
                            eq_update(&mut self.streams, vec![streams])
                        } else {
                            Effects::none().unchanged()
                        }
                    }
                    _ => Effects::none().unchanged(),
                };
                let watched_effects =
                    watched_update::<E>(&mut self.watched, &self.meta_items, &self.library_item);
                meta_items_effects
                    .join(streams_effects)
                    .join(watched_effects)
            }
            Msg::Internal(Internal::ResourceRequestResult(request, result))
                if request.path.resource == STREAM_RESOURCE_NAME =>
            {
                resources_update_with_vector_content::<E, _>(
                    &mut self.streams,
                    ResourcesAction::ResourceRequestResult {
                        request,
                        result,
                        limit: &None,
                    },
                )
            }
            Msg::Internal(Internal::LibraryChanged(_)) => {
                let library_item_effects =
                    library_item_update(&mut self.library_item, &self.selected, &ctx.library);
                let watched_effects =
                    watched_update::<E>(&mut self.watched, &self.meta_items, &self.library_item);
                library_item_effects.join(watched_effects)
            }
            _ => Effects::none().unchanged(),
        }
    }
}

fn library_item_update(
    library_item: &mut Option<LibraryItem>,
    selected: &Option<Selected>,
    library: &LibraryBucket,
) -> Effects {
    let next_library_item = match selected {
        Some(selected) => library.items.get(&selected.meta_path.id).cloned(),
        _ => None,
    };
    eq_update(library_item, next_library_item)
}

fn watched_update<E: Env>(
    watched: &mut Vec<Option<WatchedBitField>>,
    meta_items: &[ResourceLoadable<MetaItem>],
    library_item: &Option<LibraryItem>,
) -> Effects {
    let next_watched = meta_items
        .iter()
        .map(|meta_item| match &meta_item.content {
            Some(Loadable::Ready(meta_item)) => {
                let video_ids = meta_item
                    .videos
                    .iter()
                    .map(|video| &video.id)
                    .cloned()
                    .collect::<Vec<_>>();
                match library_item {
                    Some(LibraryItem {
                        state:
                            LibraryItemState {
                                watched: Some(watched),
                                ..
                            },
                        ..
                    }) => match WatchedBitField::construct_and_resize(watched, video_ids) {
                        Ok(watched) => Some(watched),
                        Err(error) => {
                            #[cfg(debug_assertions)]
                            E::log(error.to_string());
                            None
                        }
                    },
                    _ => Some(WatchedBitField::construct_from_array(vec![], video_ids)),
                }
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    eq_update(watched, next_watched)
}

fn streams_from_meta_items(
    meta_items: &[ResourceLoadable<MetaItem>],
    video_id: &str,
) -> Option<ResourceLoadable<Vec<Stream>>> {
    meta_items
        .iter()
        .find_map(|meta_item| match meta_item {
            ResourceLoadable {
                request,
                content: Some(Loadable::Ready(meta_item)),
            } => Some((request, meta_item)),
            _ => None,
        })
        .and_then(|(request, meta_item)| {
            meta_item
                .videos
                .iter()
                .find(|video| video.id == video_id && !video.streams.is_empty())
                .map(|video| (request, &video.streams))
        })
        .map(|(request, streams)| ResourceLoadable {
            request: request.to_owned(),
            content: Some(Loadable::Ready(streams.to_owned())),
        })
}
