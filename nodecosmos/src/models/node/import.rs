use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::description::Description;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::io::Io;
use crate::models::node::Node;
use crate::models::traits::{Clean, ObjectType};
use crate::models::utils::DescriptionXmlParser;
use actix_multipart::Multipart;
use charybdis::operations::{DeleteWithCallbacks, InsertWithCallbacks};
use charybdis::types::{Decimal, Double, Uuid};
use futures::StreamExt;
use serde::{Deserialize, Deserializer};
use std::collections::{HashMap, HashSet};

const TMP_ROOT_ID: &str = "root";

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
    /// Temporary id used to reference IOs in the import file
    pub id: String,
    pub title: String,
    /// For IOs with the same title, you only have to provide the description once and it will be shared
    /// across all IOs with the same title
    pub description: Option<ImportDescription>,
}

#[derive(Deserialize)]
pub struct ImportFlowStep {
    /// Temporary id used to reference flow steps in the import file or error messages
    pub id: String,
    /// Node ids that are part of the flow step. Every node in the list must be descendant of the node where
    /// the flow step is being created, or it can be equal to the node itself.
    pub node_ids: Vec<String>,
    /// Take outputs to nodes defined in current flow steps. Key is the node id that is used in the current flow step
    /// ('node_ids' list), and value is the list of Output Ids.
    pub input_ids_by_node: HashMap<String, Vec<String>>,
    /// Map outputs to nodes in the flow step. Key is the tmp node id that is used in the current flow step,
    /// and value is the tmp IO id that is used as output to the node.
    pub outputs_by_node: HashMap<String, Vec<ImportIo>>,
    /// Description of the flow step
    pub description: Option<ImportDescription>,
}

#[derive(Deserialize)]
pub struct ImportFlow {
    pub title: String,
    pub description: Option<ImportDescription>,
    /// Flow steps that are part of the flow
    pub flow_steps: Vec<ImportFlowStep>,
    /// Initial inputs to the flow
    pub initial_inputs: Vec<ImportIo>,
    /// Step where the flow starts in the complete workflow structure. For example, if we have flow step that has a node
    /// that act as a decision point, those decision can branch out to different flows. Following flows can have
    /// start_index set to the index of `current_flow.start_index + step_index + 1`.
    pub start_index: Option<i32>,
}

#[derive(Deserialize)]
pub struct ImportNode {
    /// Temporary id used to reference nodes in the import file
    pub id: String,
    pub title: String,
    pub order_index: Option<i32>,
    pub description: Option<ImportDescription>,
    /// if parent_id is 'root', then it is a top-level node where import occurs
    #[serde(default = "default_parent_id")]
    pub parent_id: String,
    pub flows: Vec<ImportFlow>,
}

fn default_parent_id() -> String {
    TMP_ROOT_ID.to_string()
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
        traverse(TMP_ROOT_ID, &mut parent_map, &mut sorted_nodes);
        self.nodes = sorted_nodes;

        self
    }
}

pub struct Import {
    pub current_root: Node,
    pub import_nodes: ImportNodes,
    pub node_id_by_tmp_id: HashMap<String, Uuid>,
    pub tmp_ids_by_node_id: HashMap<Uuid, String>,
    pub io_id_by_tmp_id: HashMap<String, Uuid>,
    pub io_id_by_title: HashMap<String, Uuid>,
    pub io_main_id_has_desc: HashMap<Uuid, bool>,
    pub io_node_id_by_id: HashMap<Uuid, Uuid>,
    pub created_flow_steps_tmp_ids: HashSet<String>,
    pub descendant_ids_by_node_id: HashMap<Uuid, HashSet<Uuid>>,
    pub created_nodes: Vec<Uuid>,
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

            let mut node_id_by_tmp_id: HashMap<String, Uuid> = HashMap::new();
            node_id_by_tmp_id.insert(TMP_ROOT_ID.to_string(), current_root.id);

            Ok(Import {
                current_root,
                import_nodes,
                node_id_by_tmp_id,
                tmp_ids_by_node_id: HashMap::new(),
                io_id_by_tmp_id: HashMap::new(),
                io_id_by_title: HashMap::new(),
                io_main_id_has_desc: HashMap::new(),
                io_node_id_by_id: HashMap::new(),
                created_flow_steps_tmp_ids: HashSet::new(),
                descendant_ids_by_node_id: HashMap::new(),
                created_nodes: Vec::new(),
            })
        } else {
            Err(NodecosmosError::ImportError("No JSON file provided".to_string()))
        }
    }

    pub async fn run(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        if let Err(e) = self.execute(data).await {
            let mut error = format!("Failed to execute import: {:?}", e);

            if let Err(e_undo) = self.undo(data).await {
                error.push_str(&format!(", Failed to undo import: {:?}", e_undo));
            }

            return Err(e);
        }

        Ok(())
    }
    async fn execute(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.insert_nodes(data).await?;
        self.insert_flows(data).await?;

        Ok(())
    }

    async fn undo(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        for node_id in self.created_nodes.iter() {
            let node = Node::maybe_find_first_by_branch_id_and_id(self.current_root.branch_id, *node_id)
                .execute(data.db_session())
                .await?;

            if let Some(mut node) = node {
                node.delete_cb(data).execute(data.db_session()).await?;
            } else {
                break;
            }
        }

        Ok(())
    }

    async fn insert_nodes(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut current_parent_id = self.current_root.id;
        let mut order_index = 0;
        let mut current_ancestors = vec![self.current_root.id];

        for import_node in self.import_nodes.nodes.iter() {
            if self.node_id_by_tmp_id.contains_key(&import_node.id) {
                return if import_node.id == TMP_ROOT_ID {
                    Err(NodecosmosError::ImportError(
                        "Can not import a node with id 'root' as 'root' refers to the node where import occurs"
                            .to_string(),
                    ))
                } else {
                    Err(NodecosmosError::ImportError(format!(
                        "Duplicate Node Id Error: Node with tmp id {} already exists",
                        import_node.id.clean_clone()
                    )))
                };
            }

            let parent_id = self
                .node_id_by_tmp_id
                .get(&import_node.parent_id)
                .copied()
                .unwrap_or_else(|| {
                    log::error!("Failed to find parent_id: {}", import_node.parent_id);
                    self.current_root.id.clone()
                });

            if import_node.parent_id == import_node.id {
                return Err(NodecosmosError::ImportError(format!(
                    "Parent Id Error: Node with tmp id {} can not be its own parent",
                    import_node.id.clean_clone()
                )));
            }

            let mut new_node = Node {
                branch_id: self.current_root.branch_id,
                root_id: self.current_root.root_id,
                parent_id: Some(parent_id),
                title: import_node.title.clone(),
                order_index: Double::from(order_index),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                ..Default::default()
            };

            new_node.insert_cb(data).execute(data.db_session()).await?;

            self.created_nodes.push(new_node.id);

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
                    base64: None,
                    updated_at: chrono::Utc::now(),
                };

                description.insert_cb(data).execute(data.db_session()).await?;
            }

            self.node_id_by_tmp_id.insert(import_node.id.clone(), new_node.id);
            self.tmp_ids_by_node_id.insert(new_node.id, import_node.id.clone());

            if parent_id != current_parent_id {
                order_index = 0;
                current_parent_id = parent_id;
            } else {
                order_index += 1;
            }

            current_ancestors.iter().for_each(|ancestor_id| {
                self.descendant_ids_by_node_id
                    .entry(ancestor_id.clone())
                    .or_insert_with(HashSet::new)
                    .insert(new_node.id.clone());
            });

            if import_node.id == TMP_ROOT_ID {
                // reset the ancestors to the root node
                current_ancestors = vec![self.current_root.id];
            } else {
                // populate ancestors as tree is built top down
                current_ancestors.push(new_node.id);
            }
        }

        Ok(())
    }

    // In order for flow steps to be able to reference IOs from different steps at the same node,
    // first, we need to create all flows, then all ios, and then all flow steps. However,
    // the problem is that we need to know flow_step_id in order to create ios. Solution is
    // to update ios with flow_step_id after all flow steps are created.
    async fn insert_flows(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        let mut import_nodes = std::mem::take(&mut self.import_nodes.nodes);
        let mut fs_flow_id_by_tmp_id: HashMap<String, Uuid> = HashMap::new();

        for import_node in import_nodes.iter() {
            let mut vertical_index = 0;

            for import_flow in import_node.flows.iter() {
                let start_index = import_flow.start_index.unwrap_or(0);

                let mut new_flow = Flow {
                    branch_id: self.current_root.branch_id,
                    root_id: self.current_root.root_id,
                    node_id: self.node_id_by_tmp_id.get(&import_node.id).copied().unwrap_or_else(|| {
                        log::error!("Failed to find node_id: {}", import_node.id);
                        self.current_root.id.clone()
                    }),
                    title: import_flow.title.clone(),
                    start_index,
                    vertical_index: Double::from(vertical_index),
                    ..Default::default()
                };

                new_flow.insert_cb(data).execute(data.db_session()).await?;

                if let Some(i_desc) = &import_flow.description {
                    let mut description = Description {
                        node_id: new_flow.node_id,
                        branch_id: new_flow.branch_id,
                        root_id: new_flow.root_id,
                        object_id: new_flow.id,
                        object_type: ObjectType::Flow.to_string(),
                        short_description: Some(i_desc.short_description.clone()),
                        html: Some(i_desc.html.clone()),
                        markdown: Some(i_desc.markdown.clone()),
                        base64: None,
                        updated_at: chrono::Utc::now(),
                    };

                    description.insert_cb(data).execute(data.db_session()).await?;
                }

                for import_io in import_flow.initial_inputs.iter() {
                    self.insert_io(data, import_io, new_flow.node_id, None).await?;
                }

                vertical_index += 1;

                // first create all ios from the flow steps and populate fs_flow_id_by_tmp_id
                for import_flow_step in import_flow.flow_steps.iter() {
                    for (_node_id, import_output) in import_flow_step.outputs_by_node.iter() {
                        for import_io in import_output.iter() {
                            self.insert_io(data, import_io, new_flow.node_id, Some(new_flow.id))
                                .await?;
                        }
                    }

                    if fs_flow_id_by_tmp_id.contains_key(&import_flow_step.id) {
                        return Err(NodecosmosError::ImportError(format!(
                            "Duplicate Flow Step Id Error: Flow Step with tmp id {} already exists",
                            import_flow_step.id.clean_clone()
                        )));
                    }
                    fs_flow_id_by_tmp_id.insert(import_flow_step.id.clone(), new_flow.id);
                }
            }
        }

        // create flow steps after all ios are created, so they can be referenced properly
        for import_node in import_nodes.iter() {
            for import_flow in import_node.flows.iter() {
                for (step_index, import_flow_step) in import_flow.flow_steps.iter().enumerate() {
                    let flow_id = fs_flow_id_by_tmp_id.get(&import_flow_step.id).copied().ok_or_else(|| {
                        log::error!("Failed to find flow_id for tmp id: {}", import_flow_step.id);
                        NodecosmosError::InternalServerError(format!(
                            "Unexpected Error: failed to find flow_id for flow_step with tmp id: {}",
                            import_flow_step.id.clean_clone()
                        ))
                    })?;

                    self.insert_flow_step(
                        data,
                        import_flow_step,
                        self.node_id_from_tmp(&import_node.id)?,
                        flow_id,
                        step_index as i32,
                    )
                    .await?;
                }
            }
        }

        self.import_nodes.nodes = std::mem::take(&mut import_nodes);

        Ok(())
    }

    async fn insert_io(
        &mut self,
        data: &RequestData,
        import_io: &ImportIo,
        node_id: Uuid,
        flow_id: Option<Uuid>,
    ) -> Result<Io, NodecosmosError> {
        if self.io_id_by_tmp_id.contains_key(&import_io.id) {
            return Err(NodecosmosError::ImportError(format!(
                "Duplicate IO Id Error: IO with tmp id {} already exists. Make sure that IOs have unique tmp ids",
                import_io.id.clean_clone()
            )));
        }

        let mut main_id = None;
        if self.io_id_by_title.contains_key(&import_io.title) {
            main_id = self.io_id_by_title.get(&import_io.title).copied();
        }

        let mut new_io = Io {
            id: Uuid::new_v4(),
            branch_id: self.current_root.branch_id,
            root_id: self.current_root.root_id,
            node_id,
            flow_id,
            title: Some(import_io.title.clone()),
            main_id,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            ctx: Default::default(),
            initial_input: flow_id.is_none(),
            ..Default::default()
        };

        new_io.insert_cb(data).execute(data.db_session()).await?;

        self.io_node_id_by_id.insert(new_io.id, node_id);
        self.io_id_by_title.insert(import_io.title.clone(), new_io.id);

        self.io_id_by_tmp_id.insert(import_io.id.clone(), new_io.id);

        if let Some(desc) = &import_io.description {
            // ios share the title and description, so if main_id has a description,
            // we don't need to insert the description again
            if main_id.map_or(false, |main_id| self.io_main_id_has_desc.get(&main_id).is_some()) {
                return Ok(new_io);
            }

            let object_id = main_id.unwrap_or(new_io.id);

            let mut description = Description {
                node_id: new_io.node_id,
                branch_id: new_io.branch_id,
                root_id: new_io.root_id,
                object_id,
                object_type: ObjectType::Io.to_string(),
                short_description: Some(desc.short_description.clone()),
                html: Some(desc.html.clone()),
                markdown: Some(desc.markdown.clone()),
                base64: None,
                updated_at: chrono::Utc::now(),
            };

            description.insert_cb(data).execute(data.db_session()).await?;

            self.io_main_id_has_desc.insert(object_id, true);
        }

        Ok(new_io)
    }

    async fn insert_flow_step(
        &mut self,
        data: &RequestData,
        import_flow_step: &ImportFlowStep,
        node_id: Uuid,
        flow_id: Uuid,
        step_index: i32,
    ) -> Result<(), NodecosmosError> {
        if self.created_flow_steps_tmp_ids.contains(&import_flow_step.id) {
            return Err(NodecosmosError::ImportError(format!(
                "Duplicate Flow Step Id Error: Flow Step with tmp id {} already exists. \
                 Make sure that Flow Steps have unique tmp ids",
                import_flow_step.id.clean_clone()
            )));
        }

        let new_fs_id = Uuid::new_v4();
        let node_ids = self.build_fs_node_ids(import_flow_step, node_id)?;
        let input_ids_by_node_id = self.build_fs_input_ids_by_node_id(import_flow_step, node_id).await?;
        let output_ids_by_node_id = self.build_fs_output_ids_by_node_id(import_flow_step).await?;

        let mut new_flow_step = FlowStep {
            id: new_fs_id,
            branch_id: self.current_root.branch_id,
            root_id: self.current_root.root_id,
            node_id,
            flow_id,
            step_index: Decimal::from(step_index),
            node_ids: Some(node_ids),
            input_ids_by_node_id: Some(input_ids_by_node_id),
            output_ids_by_node_id: Some(output_ids_by_node_id),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            ..Default::default()
        };

        new_flow_step.insert_cb(data).execute(data.db_session()).await?;
        self.created_flow_steps_tmp_ids.insert(import_flow_step.id.clone());

        // associate outputs with self as in case of import, IOs are created before flow steps
        new_flow_step.associate_outputs_with_self(data.db_session()).await?;

        if let Some(import_description) = &import_flow_step.description {
            let mut description = Description {
                node_id: new_flow_step.node_id,
                branch_id: new_flow_step.branch_id,
                root_id: new_flow_step.root_id,
                object_id: new_flow_step.id,
                object_type: ObjectType::FlowStep.to_string(),
                short_description: Some(import_description.short_description.clone()),
                html: Some(import_description.html.clone()),
                markdown: Some(import_description.markdown.clone()),
                base64: None,
                updated_at: chrono::Utc::now(),
            };

            description.insert_cb(data).execute(data.db_session()).await?;
        }

        Ok(())
    }

    // Helper method to fetch node_id
    fn node_id_from_tmp(&self, tmp_node_id: &String) -> Result<Uuid, NodecosmosError> {
        self.node_id_by_tmp_id.get(tmp_node_id).copied().ok_or_else(|| {
            log::error!("Failed to find node_id for tmp id: {}", tmp_node_id);
            NodecosmosError::ImportError(format!(
                "Failed to find node_id for tmp id: {}",
                tmp_node_id.clean_clone()
            ))
        })
    }

    fn tmp_id_from_node_id(&self, node_id: Uuid) -> Result<&String, NodecosmosError> {
        self.tmp_ids_by_node_id.get(&node_id).ok_or_else(|| {
            log::error!("Failed to find tmp id for node_id: {}", node_id);
            NodecosmosError::ImportError(format!("Failed to find tmp id for node_id: {}", node_id))
        })
    }

    // Helper method to fetch io_id
    fn io_id_from_tmp(&self, tmp_io_id: &String) -> Result<Uuid, NodecosmosError> {
        self.io_id_by_tmp_id.get(tmp_io_id).copied().ok_or_else(|| {
            log::error!("Failed to find io_id for tmp id: {}", tmp_io_id);
            NodecosmosError::ImportError(format!("Failed to find Output for tmp id: {}", tmp_io_id.clean_clone()))
        })
    }

    fn build_fs_node_ids(
        &self,
        import_flow_step: &ImportFlowStep,
        fs_node_id: Uuid,
    ) -> Result<Vec<Uuid>, NodecosmosError> {
        import_flow_step
            .node_ids
            .iter()
            .map(|tmp_node_id| {
                let node_id = self.node_id_from_tmp(tmp_node_id).map_err(|e| {
                    NodecosmosError::ImportError(format!(
                        "Flow Step {} Creation Error: {}",
                        import_flow_step.id.clean_clone(),
                        e
                    ))
                })?;

                let tmp_fs_node_id = self.tmp_id_from_node_id(fs_node_id).map_err(|e| {
                    NodecosmosError::ImportError(format!(
                        "Flow Step {} Creation Error: {}",
                        import_flow_step.id.clean_clone(),
                        e
                    ))
                })?;

                if node_id != fs_node_id
                    && self
                        .descendant_ids_by_node_id
                        .get(&fs_node_id)
                        .map_or(true, |descendants| !descendants.contains(&node_id))
                {
                    return Err(NodecosmosError::ImportError(format!(
                        "Flow Step <b>{}</b> Creation Error: \
                         Node with tmp id <b>{}</b> is not a descendant of the node with \
                         tmp id <b>{}</b> or the node itself. Please make sure that all nodes \
                         in the flow step are descendants of the node where the flow step is being created \
                         or the node itself. If you need to associate <b>sibling nodes</b>, you can create a \
                         <b>flow</b> in the <b>parent node</b>.",
                        import_flow_step.id.clean_clone(),
                        tmp_node_id.clean_clone(),
                        tmp_fs_node_id.clean_clone()
                    )));
                }

                Ok(node_id)
            })
            .collect::<Result<Vec<Uuid>, NodecosmosError>>()
    }

    async fn build_fs_input_ids_by_node_id(
        &self,
        import_flow_step: &ImportFlowStep,
        node_id: Uuid,
    ) -> Result<HashMap<Uuid, Vec<Uuid>>, NodecosmosError> {
        import_flow_step
            .input_ids_by_node
            .iter()
            .map(|(tmp_node_id, tmp_io_ids)| {
                let input_node_id = self.node_id_from_tmp(tmp_node_id).map_err(|e| {
                    NodecosmosError::ImportError(format!(
                        "Flow Step {} Creation Error: {}",
                        import_flow_step.id.clean_clone(),
                        e
                    ))
                })?;

                let io_ids = tmp_io_ids
                    .iter()
                    .map(|tmp_io_id| {
                        let io_id = self.io_id_from_tmp(tmp_io_id);
                        // ensure that IO is in the scope
                        if let Ok(io_id) = io_id {
                            let io_node_id = self.io_node_id_by_id.get(&io_id).ok_or_else(|| {
                                NodecosmosError::InternalServerError(format!(
                                    "Unexpected Error: IO with tmp id {} and id {} is not associated with any node",
                                    tmp_io_id, io_id
                                ))
                            })?;

                            if io_node_id != &node_id {
                                return Err(NodecosmosError::ImportError(format!(
                                    "Flow Step {} Creation Error: IO with tmp id {} that is used as input is not in \
                                    the scope of the flow step. Please make sure that IOs associate inputs that are \
                                    created in the flows within the same node",
                                    import_flow_step.id.clean_clone(),
                                    tmp_io_id.clean_clone()
                                )));
                            }
                        }

                        io_id
                    })
                    .collect::<Result<Vec<Uuid>, NodecosmosError>>()?;

                Ok((input_node_id, io_ids))
            })
            .collect::<Result<HashMap<Uuid, Vec<Uuid>>, NodecosmosError>>()
    }

    async fn build_fs_output_ids_by_node_id(
        &mut self,
        import_flow_step: &ImportFlowStep,
    ) -> Result<HashMap<Uuid, Vec<Uuid>>, NodecosmosError> {
        let mut output_ids_by_node_id = HashMap::new();

        for (tmp_node_id, import_ios) in import_flow_step.outputs_by_node.iter() {
            let mut io_ids = Vec::new();

            for import_io in import_ios.iter() {
                let io_id = self.io_id_from_tmp(&import_io.id).map_err(|e| {
                    NodecosmosError::ImportError(format!(
                        "Flow Step {} Creation Error: {}",
                        import_flow_step.id.clean_clone(),
                        e
                    ))
                })?;

                io_ids.push(io_id);
            }

            let output_node_id = self.node_id_from_tmp(tmp_node_id).map_err(|e| {
                NodecosmosError::ImportError(format!(
                    "Flow Step {} Creation Error: {}",
                    import_flow_step.id.clean_clone(),
                    e
                ))
            })?;

            output_ids_by_node_id.insert(output_node_id, io_ids);
        }

        Ok(output_ids_by_node_id)
    }
}
