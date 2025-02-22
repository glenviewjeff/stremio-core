use crate::runtime::Env;
use crate::types::resource::{MetaItemBehaviorHints, MetaItemPreview, PosterShape, Video};
use chrono::{DateTime, Duration, Utc};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnError, DefaultOnNull, NoneAsEmptyString};
use std::marker::PhantomData;
use stremio_watched_bitfield::WatchedBitField;
use url::Url;

#[serde_as]
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct LibraryItem {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub r#type: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError<NoneAsEmptyString>")]
    pub poster: Option<Url>,
    #[serde(default)]
    pub poster_shape: PosterShape,
    pub removed: bool,
    pub temp: bool,
    #[serde(default, rename = "_ctime")]
    #[serde_as(deserialize_as = "DefaultOnNull<NoneAsEmptyString>")]
    pub ctime: Option<DateTime<Utc>>,
    #[serde(rename = "_mtime")]
    pub mtime: DateTime<Utc>,
    pub state: LibraryItemState,
    #[serde(default)]
    pub behavior_hints: MetaItemBehaviorHints,
}

impl LibraryItem {
    #[inline]
    pub fn should_sync<E: Env + 'static>(&self) -> bool {
        let year_ago = E::now() - Duration::days(365);
        let recently_removed = self.removed && self.mtime > year_ago;
        self.r#type != "other" && (!self.removed || recently_removed)
    }
    #[inline]
    pub fn is_in_continue_watching(&self) -> bool {
        self.r#type != "other" && (!self.removed || self.temp) && self.state.time_offset > 0
    }
}

impl<E: Env + 'static> From<(&MetaItemPreview, PhantomData<E>)> for LibraryItem {
    fn from((meta_item, _): (&MetaItemPreview, PhantomData<E>)) -> Self {
        LibraryItem {
            id: meta_item.id.to_owned(),
            removed: true,
            temp: true,
            ctime: Some(E::now()),
            mtime: E::now(),
            state: LibraryItemState::default(),
            name: meta_item.name.to_owned(),
            r#type: meta_item.r#type.to_owned(),
            poster: meta_item.poster.to_owned(),
            poster_shape: meta_item.poster_shape.to_owned(),
            behavior_hints: meta_item.behavior_hints.to_owned(),
        }
    }
}

impl From<(&MetaItemPreview, &LibraryItem)> for LibraryItem {
    fn from((meta_item, library_item): (&MetaItemPreview, &LibraryItem)) -> Self {
        LibraryItem {
            id: meta_item.id.to_owned(),
            name: meta_item.name.to_owned(),
            r#type: meta_item.r#type.to_owned(),
            poster: meta_item.poster.to_owned(),
            poster_shape: meta_item.poster_shape.to_owned(),
            behavior_hints: meta_item.behavior_hints.to_owned(),
            removed: library_item.removed,
            temp: library_item.temp,
            ctime: library_item.ctime.to_owned(),
            mtime: library_item.mtime.to_owned(),
            state: library_item.state.to_owned(),
        }
    }
}

#[serde_as]
#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct LibraryItemState {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnNull<NoneAsEmptyString>")]
    pub last_watched: Option<DateTime<Utc>>,
    pub time_watched: u64,
    pub time_offset: u64,
    pub overall_time_watched: u64,
    pub times_watched: u32,
    // @TODO: consider bool that can be deserialized from an integer
    pub flagged_watched: u32,
    pub duration: u64,
    #[serde(default, rename = "video_id")]
    #[serde_as(deserialize_as = "DefaultOnNull<NoneAsEmptyString>")]
    pub video_id: Option<String>,
    // @TODO bitfield, special type
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnNull<NoneAsEmptyString>")]
    pub watched: Option<String>,
    // release date of last observed video
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnNull<NoneAsEmptyString>")]
    pub last_vid_released: Option<DateTime<Utc>>,
    pub no_notif: bool,
}

impl LibraryItemState {
    pub fn watched_bitfield(&self, videos: &[Video]) -> WatchedBitField {
        let video_ids = videos
            .iter()
            .sorted_by(|a, b| {
                a.series_info
                    .as_ref()
                    .map(|info| info.season as i64)
                    .unwrap_or(i64::MIN)
                    .cmp(
                        &b.series_info
                            .as_ref()
                            .map(|info| info.season as i64)
                            .unwrap_or(i64::MIN),
                    )
                    .then(
                        a.series_info
                            .as_ref()
                            .map(|info| info.episode as i64)
                            .unwrap_or(i64::MIN)
                            .cmp(
                                &b.series_info
                                    .as_ref()
                                    .map(|info| info.episode as i64)
                                    .unwrap_or(i64::MIN),
                            ),
                    )
                    .then(
                        a.released
                            .as_ref()
                            .map(|released| released.timestamp_millis())
                            .unwrap_or(i64::MIN)
                            .cmp(
                                &b.released
                                    .as_ref()
                                    .map(|released| released.timestamp_millis())
                                    .unwrap_or(i64::MIN),
                            ),
                    )
            })
            .map(|video| &video.id)
            .cloned()
            .collect::<Vec<_>>();
        match &self.watched {
            Some(watched) => {
                match WatchedBitField::construct_and_resize(watched, video_ids.to_owned()) {
                    Ok(watched) => watched,
                    Err(_) => WatchedBitField::construct_from_array(vec![], video_ids),
                }
            }
            _ => WatchedBitField::construct_from_array(vec![], video_ids),
        }
    }
}
