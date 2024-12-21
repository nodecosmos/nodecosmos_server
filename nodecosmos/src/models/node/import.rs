use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::description::Description;
use crate::models::node::Node;
use crate::models::traits::ObjectType;
use crate::models::utils::DescriptionXmlParser;
use actix_multipart::Multipart;
use charybdis::operations::InsertWithCallbacks;
use charybdis::types::{Double, Uuid};
use futures::StreamExt;
use serde::{Deserialize, Deserializer};
use std::collections::{HashMap, HashSet};

pub struct ImportDescription {
    pub html: String,
    pub markdown: String,
    pub short_description: String,
}

impl From<String> for ImportDescription {
    fn from(xml: String) -> ImportDescription {
        let parsed = DescriptionXmlParser::new(&xml).run();

        match parsed {
            Ok(parser) => ImportDescription {
                html: parser.html,
                markdown: parser.markdown,
                short_description: parser.short_description,
            },
            Err(e) => {
                log::error!("Failed to parse description XML: {}", e);

                ImportDescription {
                    html: format!("Failed to parse description XML: {}", e).to_string(),
                    markdown: format!("Failed to parse description XML: {}", e).to_string(),
                    short_description: String::new(),
                }
            }
        }
    }
}

impl<'de> Deserialize<'de> for ImportDescription {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        Ok(ImportDescription::from(s))
    }
}

#[derive(Deserialize)]
pub struct ImportIo {
    pub id: String,
    pub title: String,
    // note that for IOs with the same title, you only have to provide the description once
    pub description: Option<ImportDescription>,
}

#[derive(Deserialize)]
pub struct ImportFlowStep {
    /// nodes of the flow step must be descendants of the node where the flow is created or the node itself
    pub node_ids: Vec<String>,
    /// input ids can come from any IO that is part of the flows in the current node
    pub input_ids_by_node: HashMap<String, Vec<String>>,
    /// output ids can be used by any IO that is part of the flows in the current node
    pub outputs_by_node: HashMap<String, Vec<ImportIo>>,
    pub description: Option<ImportDescription>,
}

#[derive(Deserialize)]
pub struct ImportFlow {
    pub title: String,
    pub description: Option<ImportDescription>,
    pub flow_steps: Vec<ImportFlowStep>,
    pub initial_inputs: Vec<ImportIo>,
}

#[derive(Deserialize)]
pub struct ImportNode {
    pub id: String,
    pub title: String,
    pub order_index: Option<i32>,
    pub description: Option<ImportDescription>,
    /// if parent_id is 'root', then it is a top-level node where import occurs
    pub parent_id: String,
    pub flows: Vec<ImportFlow>,
}

#[derive(Deserialize)]
pub struct ImportNodes {
    pub nodes: Vec<ImportNode>,
}

impl ImportNodes {
    pub fn sort(mut self) -> Self {
        // Step 1: Build a map from parent_id to its children
        let mut parent_map: HashMap<String, Vec<ImportNode>> = HashMap::new();
        for node in self.nodes.drain(..) {
            parent_map
                .entry(node.parent_id.clone())
                .or_insert_with(Vec::new)
                .push(node);
        }

        // Step 2: Define a recursive function to traverse and collect sorted nodes
        fn traverse(parent_id: &str, parent_map: &mut HashMap<String, Vec<ImportNode>>, sorted: &mut Vec<ImportNode>) {
            if let Some(mut children) = parent_map.remove(parent_id) {
                // Sort children by order_index if present, else leave them as is
                children.sort_by(|a, b| match (a.order_index, b.order_index) {
                    (Some(a_idx), Some(b_idx)) => a_idx.cmp(&b_idx),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                });

                for child in children {
                    let child_id = child.id.clone();
                    sorted.push(child);
                    traverse(&child_id, parent_map, sorted);
                }
            }
        }

        // Step 3: Start traversal from 'root'
        let mut sorted_nodes: Vec<ImportNode> = Vec::new();
        traverse("root", &mut parent_map, &mut sorted_nodes);
        self.nodes = sorted_nodes;

        self
    }
}

pub struct Import {
    pub current_root: Node,
    pub import_nodes: ImportNodes,
    pub ids_by_tmp_id: HashMap<String, Uuid>,
    pub nodes_by_tmp_id: HashMap<String, Node>,
    pub ios_by_tmp_id: HashMap<String, Node>,
    pub ios_by_title: HashMap<String, Node>,
    pub flows_by_tmp_id: HashMap<String, Node>,
    pub descendant_ids_by_node_id: HashMap<String, HashSet<String>>,
}

impl Import {
    pub async fn new(current_root: Node, mut json_file: Multipart) -> Result<Import, NodecosmosError> {
        if let Some(item) = json_file.next().await {
            let mut field = item.map_err(|e| {
                NodecosmosError::InternalServerError(format!("Failed to read multipart field: {:?}", e))
            })?;

            // get the JSON from the file
            let mut json = String::new();
            while let Some(chunk) = field.next().await {
                let data = chunk
                    .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to read chunk: {:?}", e)))?;

                json.push_str(&String::from_utf8(data.to_vec()).map_err(|e| {
                    NodecosmosError::InternalServerError(format!("Failed to convert chunk to string: {:?}", e))
                })?);
            }

            let import_nodes: ImportNodes = serde_json::from_str::<ImportNodes>(&json)
                .map_err(|e| NodecosmosError::InternalServerError(format!("Failed to parse JSON: {:?}", e)))?
                .sort();

            let mut ids_by_tmp_id: HashMap<String, Uuid> = HashMap::new();
            ids_by_tmp_id.insert("root".to_string(), current_root.id);

            let mut nodes_by_tmp_id: HashMap<String, Node> = HashMap::new();
            nodes_by_tmp_id.insert("root".to_string(), current_root.clone());

            Ok(Import {
                current_root,
                import_nodes,
                ids_by_tmp_id,
                nodes_by_tmp_id: HashMap::new(),
                ios_by_tmp_id: HashMap::new(),
                ios_by_title: HashMap::new(),
                flows_by_tmp_id: HashMap::new(),
                descendant_ids_by_node_id: HashMap::new(),
            })
        } else {
            Err(NodecosmosError::BadRequest("No JSON file provided".to_string()))
        }
    }

    pub async fn run(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.create_nodes(data).await?;

        Ok(())
    }

    async fn create_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut current_parent_id = self.current_root.id;
        let mut order_index = 0;
        for import_node in self.import_nodes.nodes.iter() {
            let parent_id = self.ids_by_tmp_id[&import_node.parent_id];
            let mut new_node = Node {
                branch_id: self.current_root.branch_id,
                root_id: self.current_root.root_id,
                parent_id: Some(parent_id),
                title: import_node.title.clone(),
                order_index: Double::from(order_index),
                ..Default::default()
            };

            new_node.insert_cb(data).execute(data.db_session()).await?;

            if let Some(i_desc) = &import_node.description {
                let mut description = Description {
                    node_id: new_node.id,
                    branch_id: new_node.branch_id,
                    root_id: new_node.root_id,
                    object_id: new_node.id,
                    object_type: ObjectType::Node.to_string(),
                    short_description: Some(i_desc.short_description.clone()),
                    html: Some(i_desc.html.clone()),
                    markdown: Some(i_desc.markdown.clone()),
                    ..Default::default()
                };

                description.insert_cb(data).execute(data.db_session()).await?;
            }

            self.ids_by_tmp_id.insert(import_node.id.clone(), new_node.id);
            self.nodes_by_tmp_id.insert(import_node.id.clone(), new_node);

            if parent_id != current_parent_id {
                order_index = 0;
                current_parent_id = parent_id;
            } else {
                order_index += 1;
            }
        }

        Ok(())
    }
}
