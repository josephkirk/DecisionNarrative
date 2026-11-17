use crate::{chrono_participants::{KnowledgeRow,PartIx}, run::{RunAction,SceneView,PlaceView}};
use serde::{Deserialize,Serialize};
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Observation { pub actor:PartIx, pub place:Option<PlaceView>, pub co_present:Vec<PartIx>, pub scene:Option<SceneView>, pub available_actions:Vec<RunAction>, pub knowledge:Vec<KnowledgeRow>, pub witnessed_events:Vec<u64> }
