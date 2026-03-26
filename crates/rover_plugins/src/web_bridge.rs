//! Web plugin bridge: JavaScript API injected into WKWebView for web plugins.
//!
//! This module provides the `window.rover` bridge that web/JS/TS plugins
//! use to communicate with Rover. The bridge script is injected as a
//! WKUserScript at document-start before the plugin's own scripts run.

/// The JavaScript bridge code injected into web plugin WKWebViews.
/// Provides `window.rover.*` API for query handling, result submission,
/// storage access, and action dispatch.
pub const BRIDGE_SCRIPT: &str = r#"
(function() {
    'use strict';

    // Internal state
    const _callbacks = {
        onQuery: [],
        onConfirm: [],
        onConfig: [],
    };
    let _pluginId = null;

    // Message handler — called by native code via evaluateJavaScript
    window.__rover_dispatch = function(event) {
        switch (event.type) {
            case 'query':
                _callbacks.onQuery.forEach(cb => cb(event.text));
                break;
            case 'confirm':
                _callbacks.onConfirm.forEach(cb => cb(event.match_id, event.secondary || false));
                break;
            case 'config':
                _callbacks.onConfig.forEach(cb => cb(event.data));
                break;
            case 'storage_value':
                // Resolve pending storage.get promise
                if (_pendingStorage[event.key]) {
                    _pendingStorage[event.key].resolve(event.value);
                    delete _pendingStorage[event.key];
                }
                break;
        }
    };

    const _pendingStorage = {};
    let _storageId = 0;

    // Post a message to native Rover
    function postMessage(msg) {
        if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.rover) {
            window.webkit.messageHandlers.rover.postMessage(JSON.stringify(msg));
        }
    }

    // Public API
    window.rover = {
        // Register as a plugin
        ready: function(pluginId) {
            _pluginId = pluginId;
            postMessage({ type: 'ready', plugin_id: pluginId });
        },

        // Event listeners
        onQuery: function(callback) {
            _callbacks.onQuery.push(callback);
        },

        onConfirm: function(callback) {
            _callbacks.onConfirm.push(callback);
        },

        onConfig: function(callback) {
            _callbacks.onConfig.push(callback);
        },

        // Submit results to the picker
        addResults: function(results) {
            postMessage({
                type: 'add_results',
                results: results.map(r => ({
                    id: r.id || '',
                    title: r.title || '',
                    subtitle: r.subtitle || null,
                    icon: r.icon || null,
                    score: r.score || 0.5,
                })),
            });
        },

        // Clear previous results
        clearResults: function() {
            postMessage({ type: 'clear_results' });
        },

        // Actions
        dismiss: function() {
            postMessage({ type: 'dismiss' });
        },

        copyToClipboard: function(text) {
            postMessage({ type: 'copy_to_clipboard', text: text });
        },

        openUrl: function(url) {
            postMessage({ type: 'open_url', url: url });
        },

        runShell: function(command) {
            postMessage({ type: 'run_shell', command: command });
        },

        // Key-value storage (async)
        storage: {
            get: function(key) {
                return new Promise(function(resolve) {
                    _pendingStorage[key] = { resolve: resolve };
                    postMessage({ type: 'storage_get', key: key });
                });
            },

            set: function(key, value) {
                postMessage({ type: 'storage_set', key: key, value: value });
                return Promise.resolve();
            },

            delete: function(key) {
                postMessage({ type: 'storage_delete', key: key });
                return Promise.resolve();
            },
        },

        // Utility
        log: function(message) {
            postMessage({ type: 'log', message: String(message) });
        },
    };

    // Signal bridge is ready
    postMessage({ type: 'bridge_ready' });
})();
"#;

/// Parse a JSON message from the web plugin bridge.
/// Returns a structured `WebBridgeMessage` or an error.
pub fn parse_bridge_message(json: &str) -> Result<WebBridgeMessage, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("Invalid JSON: {}", e))?;

    let msg_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'type' field")?;

    match msg_type {
        "bridge_ready" => Ok(WebBridgeMessage::BridgeReady),
        "ready" => {
            let plugin_id = value
                .get("plugin_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(WebBridgeMessage::PluginReady { plugin_id })
        }
        "add_results" => {
            let results = value
                .get("results")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|r| WebBridgeResult {
                            id: r.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            title: r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            subtitle: r.get("subtitle").and_then(|v| v.as_str()).map(String::from),
                            score: r.get("score").and_then(|v| v.as_f64()).unwrap_or(0.5),
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(WebBridgeMessage::AddResults { results })
        }
        "clear_results" => Ok(WebBridgeMessage::ClearResults),
        "dismiss" => Ok(WebBridgeMessage::Dismiss),
        "copy_to_clipboard" => {
            let text = value.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(WebBridgeMessage::CopyToClipboard(text))
        }
        "open_url" => {
            let url = value.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(WebBridgeMessage::OpenUrl(url))
        }
        "run_shell" => {
            let cmd = value.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(WebBridgeMessage::RunShell(cmd))
        }
        "storage_get" => {
            let key = value.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(WebBridgeMessage::StorageGet { key })
        }
        "storage_set" => {
            let key = value.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let val = value.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(WebBridgeMessage::StorageSet { key, value: val })
        }
        "storage_delete" => {
            let key = value.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(WebBridgeMessage::StorageDelete { key })
        }
        "log" => {
            let message = value.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(WebBridgeMessage::Log(message))
        }
        other => Err(format!("Unknown message type: {}", other)),
    }
}

/// A parsed message from the web plugin bridge.
#[derive(Debug, Clone, PartialEq)]
pub enum WebBridgeMessage {
    BridgeReady,
    PluginReady { plugin_id: String },
    AddResults { results: Vec<WebBridgeResult> },
    ClearResults,
    Dismiss,
    CopyToClipboard(String),
    OpenUrl(String),
    RunShell(String),
    StorageGet { key: String },
    StorageSet { key: String, value: String },
    StorageDelete { key: String },
    Log(String),
}

/// A match result from a web plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct WebBridgeResult {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub score: f64,
}

/// Generate JavaScript to dispatch an event to the web plugin.
pub fn dispatch_query_js(query: &str) -> String {
    let escaped = query.replace('\\', "\\\\").replace('\'', "\\'").replace('\n', "\\n");
    format!(
        "window.__rover_dispatch({{ type: 'query', text: '{}' }});",
        escaped
    )
}

/// Generate JavaScript to dispatch a confirm event.
pub fn dispatch_confirm_js(match_id: &str, secondary: bool) -> String {
    let escaped = match_id.replace('\\', "\\\\").replace('\'', "\\'");
    format!(
        "window.__rover_dispatch({{ type: 'confirm', match_id: '{}', secondary: {} }});",
        escaped, secondary
    )
}

/// Generate JavaScript to send a storage value response.
pub fn dispatch_storage_value_js(key: &str, value: Option<&str>) -> String {
    let escaped_key = key.replace('\\', "\\\\").replace('\'', "\\'");
    match value {
        Some(v) => {
            let escaped_val = v.replace('\\', "\\\\").replace('\'', "\\'");
            format!(
                "window.__rover_dispatch({{ type: 'storage_value', key: '{}', value: '{}' }});",
                escaped_key, escaped_val
            )
        }
        None => {
            format!(
                "window.__rover_dispatch({{ type: 'storage_value', key: '{}', value: null }});",
                escaped_key
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bridge_ready() {
        let msg = parse_bridge_message(r#"{"type":"bridge_ready"}"#).unwrap();
        assert_eq!(msg, WebBridgeMessage::BridgeReady);
    }

    #[test]
    fn test_parse_plugin_ready() {
        let msg = parse_bridge_message(r#"{"type":"ready","plugin_id":"ext.weather"}"#).unwrap();
        assert_eq!(
            msg,
            WebBridgeMessage::PluginReady {
                plugin_id: "ext.weather".to_string()
            }
        );
    }

    #[test]
    fn test_parse_add_results() {
        let json = r#"{"type":"add_results","results":[{"id":"r1","title":"Rain","subtitle":"80%","score":0.9}]}"#;
        let msg = parse_bridge_message(json).unwrap();
        match msg {
            WebBridgeMessage::AddResults { results } => {
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].id, "r1");
                assert_eq!(results[0].title, "Rain");
                assert_eq!(results[0].subtitle, Some("80%".to_string()));
                assert!((results[0].score - 0.9).abs() < 0.01);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_parse_clear_results() {
        let msg = parse_bridge_message(r#"{"type":"clear_results"}"#).unwrap();
        assert_eq!(msg, WebBridgeMessage::ClearResults);
    }

    #[test]
    fn test_parse_copy_to_clipboard() {
        let msg =
            parse_bridge_message(r#"{"type":"copy_to_clipboard","text":"hello"}"#).unwrap();
        assert_eq!(msg, WebBridgeMessage::CopyToClipboard("hello".to_string()));
    }

    #[test]
    fn test_parse_storage_set() {
        let msg =
            parse_bridge_message(r#"{"type":"storage_set","key":"k","value":"v"}"#).unwrap();
        assert_eq!(
            msg,
            WebBridgeMessage::StorageSet {
                key: "k".to_string(),
                value: "v".to_string()
            }
        );
    }

    #[test]
    fn test_parse_unknown_type() {
        let result = parse_bridge_message(r#"{"type":"foobar"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_dispatch_query_js() {
        let js = dispatch_query_js("hello world");
        assert!(js.contains("hello world"));
        assert!(js.contains("__rover_dispatch"));
    }

    #[test]
    fn test_dispatch_query_js_escapes() {
        let js = dispatch_query_js("it's a \"test\"\nline2");
        assert!(js.contains("it\\'s a "));
        assert!(js.contains("\\n"));
    }

    #[test]
    fn test_dispatch_confirm_js() {
        let js = dispatch_confirm_js("match-1", true);
        assert!(js.contains("match-1"));
        assert!(js.contains("true"));
    }

    #[test]
    fn test_dispatch_storage_value_js_some() {
        let js = dispatch_storage_value_js("key1", Some("val1"));
        assert!(js.contains("key1"));
        assert!(js.contains("val1"));
    }

    #[test]
    fn test_dispatch_storage_value_js_none() {
        let js = dispatch_storage_value_js("key1", None);
        assert!(js.contains("null"));
    }
}
