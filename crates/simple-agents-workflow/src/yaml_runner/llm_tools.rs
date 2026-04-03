use serde_json::{json, Value};
use simple_agent_type::tool::{
    ToolChoice, ToolChoiceFunction, ToolChoiceTool, ToolDefinition, ToolFunction, ToolType,
};

use super::{
    YamlLlmCall, YamlNode, YamlResolvedTool, YamlToolChoiceConfig, YamlToolDeclaration,
    YamlToolFormat,
};

pub(super) fn llm_output_schema_for_node(node: &YamlNode) -> Value {
    if let Some(schema) = node
        .config
        .as_ref()
        .and_then(|cfg| cfg.output_schema.clone())
    {
        return schema;
    }

    default_llm_output_schema()
}

pub(super) fn normalize_tool_choice(
    config: Option<YamlToolChoiceConfig>,
) -> Result<Option<ToolChoice>, String> {
    let Some(config) = config else {
        return Ok(None);
    };

    let choice = match config {
        YamlToolChoiceConfig::Mode(mode) => ToolChoice::Mode(mode),
        YamlToolChoiceConfig::Function(function) => ToolChoice::Tool(ToolChoiceTool {
            tool_type: ToolType::Function,
            function: ToolChoiceFunction {
                name: function.function,
            },
        }),
        YamlToolChoiceConfig::OpenAi(tool) => ToolChoice::Tool(tool),
    };

    Ok(Some(choice))
}

pub(super) fn normalize_llm_tools(llm: &YamlLlmCall) -> Result<Vec<YamlResolvedTool>, String> {
    llm.tools
        .iter()
        .map(|tool| match (llm.tools_format, tool) {
            (YamlToolFormat::Openai, YamlToolDeclaration::OpenAi(openai)) => {
                let definition = ToolDefinition {
                    tool_type: openai.tool_type.unwrap_or(ToolType::Function),
                    function: ToolFunction {
                        name: openai.function.name.clone(),
                        description: openai.function.description.clone(),
                        parameters: openai.function.parameters.clone(),
                    },
                };
                Ok(YamlResolvedTool {
                    definition,
                    output_schema: openai.function.output_schema.clone(),
                })
            }
            (YamlToolFormat::Simplified, YamlToolDeclaration::Simplified(simple)) => {
                let definition = ToolDefinition {
                    tool_type: ToolType::Function,
                    function: ToolFunction {
                        name: simple.name.clone(),
                        description: simple.description.clone(),
                        parameters: Some(simple.input_schema.clone()),
                    },
                };
                Ok(YamlResolvedTool {
                    definition,
                    output_schema: simple.output_schema.clone(),
                })
            }
            (YamlToolFormat::Openai, _) => {
                Err("tools_format=openai requires OpenAI-style tool declarations".to_string())
            }
            (YamlToolFormat::Simplified, _) => {
                Err("tools_format=simplified requires simplified tool declarations".to_string())
            }
        })
        .collect()
}

pub(super) fn default_llm_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": true
    })
}
