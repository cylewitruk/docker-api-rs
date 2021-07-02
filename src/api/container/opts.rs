use crate::api::Labels;

use std::{collections::HashMap, hash::Hash, iter::Peekable, str, time::Duration};

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::{Error, Result};

/// Filter Opts for container listings
pub enum ContainerFilter {
    ExitCode(u64),
    Status(String),
    LabelName(String),
    Label(String, String),
}

impl_url_opts_builder!(derives = Default | ContainerList);

impl ContainerListOptsBuilder {
    pub fn filter<F>(&mut self, filters: F) -> &mut Self
    where
        F: IntoIterator<Item = ContainerFilter>,
    {
        let mut param = HashMap::new();
        for f in filters {
            match f {
                ContainerFilter::ExitCode(c) => param.insert("exit", vec![c.to_string()]),
                ContainerFilter::Status(s) => param.insert("status", vec![s]),
                ContainerFilter::LabelName(n) => param.insert("label", vec![n]),
                ContainerFilter::Label(n, v) => param.insert("label", vec![format!("{}={}", n, v)]),
            };
        }
        // structure is a a json encoded object mapping string keys to a list
        // of string values
        self.params
            .insert("filters", serde_json::to_string(&param).unwrap_or_default());
        self
    }

    impl_url_bool_field!("If set to true all containers will be returned" all => "all");

    impl_url_str_field!(since: S => "since");

    impl_url_str_field!(before: B => "before");

    impl_url_bool_field!("If set to true the sizes of the containers will be returned" sized => "size");
}

/// Interface for building a new docker container from an existing image
#[derive(Serialize, Debug)]
pub struct ContainerOpts {
    pub name: Option<String>,
    params: HashMap<&'static str, Value>,
}

/// Function to insert a JSON value into a tree where the desired
/// location of the value is given as a path of JSON keys.
fn insert<'a, I, V>(key_path: &mut Peekable<I>, value: &V, parent_node: &mut Value)
where
    V: Serialize,
    I: Iterator<Item = &'a str>,
{
    if let Some(local_key) = key_path.next() {
        if key_path.peek().is_some() {
            if let Some(node) = parent_node.as_object_mut() {
                let node = node
                    .entry(local_key.to_string())
                    .or_insert(Value::Object(Map::new()));

                insert(key_path, value, node);
            }
        } else if let Some(node) = parent_node.as_object_mut() {
            node.insert(
                local_key.to_string(),
                serde_json::to_value(value).unwrap_or_default(),
            );
        }
    }
}

impl ContainerOpts {
    /// return a new instance of a builder for Opts
    pub fn builder<N>(name: N) -> ContainerOptsBuilder
    where
        N: AsRef<str>,
    {
        ContainerOptsBuilder::new(name.as_ref())
    }

    /// serialize Opts as a string. returns None if no Opts are defined
    pub fn serialize(&self) -> Result<String> {
        serde_json::to_string(&self.to_json()).map_err(Error::from)
    }

    fn to_json(&self) -> Value {
        let mut body_members = Map::new();
        // The HostConfig element gets initialized to an empty object,
        // for backward compatibility.
        body_members.insert("HostConfig".to_string(), Value::Object(Map::new()));
        let mut body = Value::Object(body_members);
        self.parse_from(&self.params, &mut body);
        body
    }

    pub fn parse_from<'a, K, V>(&self, params: &'a HashMap<K, V>, body: &mut Value)
    where
        &'a HashMap<K, V>: IntoIterator,
        K: ToString + Eq + Hash,
        V: Serialize,
    {
        for (k, v) in params.iter() {
            let key_string = k.to_string();
            insert(&mut key_string.split('.').peekable(), v, body)
        }
    }
}

#[derive(Default)]
pub struct ContainerOptsBuilder {
    name: Option<String>,
    params: HashMap<&'static str, Value>,
}

impl ContainerOptsBuilder {
    pub(crate) fn new(image: &str) -> Self {
        let mut params = HashMap::new();

        params.insert("Image", Value::String(image.to_owned()));
        ContainerOptsBuilder { name: None, params }
    }

    pub fn name<N>(&mut self, name: N) -> &mut Self
    where
        N: Into<String>,
    {
        self.name = Some(name.into());
        self
    }

    /// enable all exposed ports on the container to be mapped to random, available, ports on the host
    pub fn publish_all_ports(&mut self) -> &mut Self {
        self.params
            .insert("HostConfig.PublishAllPorts", Value::Bool(true));
        self
    }

    pub fn expose<P>(&mut self, srcport: u32, protocol: P, hostport: u32) -> &mut Self
    where
        P: AsRef<str>,
    {
        let mut exposedport: Labels = HashMap::new();
        exposedport.insert("HostPort".to_string(), hostport.to_string());

        // The idea here is to go thought the 'old' port binds and to apply them to the local
        // 'port_bindings' variable, add the bind we want and replace the 'old' value
        let mut port_bindings: HashMap<String, Value> = HashMap::new();
        for (key, val) in self
            .params
            .get("HostConfig.PortBindings")
            .unwrap_or(&json!(null))
            .as_object()
            .unwrap_or(&Map::new())
            .iter()
        {
            port_bindings.insert(key.to_string(), json!(val));
        }
        port_bindings.insert(
            format!("{}/{}", srcport, protocol.as_ref()),
            json!(vec![exposedport]),
        );

        self.params
            .insert("HostConfig.PortBindings", json!(port_bindings));

        // Replicate the port bindings over to the exposed ports config
        let mut exposed_ports: HashMap<String, Value> = HashMap::new();
        let empty_config: HashMap<String, Value> = HashMap::new();
        for key in port_bindings.keys() {
            exposed_ports.insert(key.to_string(), json!(empty_config));
        }

        self.params.insert("ExposedPorts", json!(exposed_ports));

        self
    }

    /// Publish a port in the container without assigning a port on the host
    pub fn publish<P>(&mut self, srcport: u32, protocol: P) -> &mut Self
    where
        P: AsRef<str>,
    {
        /* The idea here is to go thought the 'old' port binds
         * and to apply them to the local 'exposedport_bindings' variable,
         * add the bind we want and replace the 'old' value */
        let mut exposed_port_bindings: HashMap<String, Value> = HashMap::new();
        for (key, val) in self
            .params
            .get("ExposedPorts")
            .unwrap_or(&json!(null))
            .as_object()
            .unwrap_or(&Map::new())
            .iter()
        {
            exposed_port_bindings.insert(key.to_string(), json!(val));
        }
        exposed_port_bindings.insert(format!("{}/{}", srcport, protocol.as_ref()), json!({}));

        // Replicate the port bindings over to the exposed ports config
        let mut exposed_ports: HashMap<String, Value> = HashMap::new();
        let empty_config: HashMap<String, Value> = HashMap::new();
        for key in exposed_port_bindings.keys() {
            exposed_ports.insert(key.to_string(), json!(empty_config));
        }

        self.params.insert("ExposedPorts", json!(exposed_ports));

        self
    }

    impl_str_field!(
    "Specify the working dir (corresponds to the `-w` docker cli argument)"
    working_dir: W => "WorkingDir");

    impl_vec_field!(
        "Specify any bind mounts, taking the form of `/some/host/path:/some/container/path`"
        volumes: V => "HostConfig.Binds"
    );

    impl_vec_field!(links: L => "HostConfig.Links");

    impl_field!(memory: u64 => "HostConfig.Memory");

    impl_field!(
    "Total memory limit (memory + swap) in bytes. Set to -1 (default) to enable unlimited swap."
    memory_swap: i64 => "HostConfig.MemorySwap");

    impl_field!(
    "CPU quota in units of 10<sup>-9</sup> CPUs. Set to 0 (default) for there to be no limit."
    ""
    "For example, setting `nano_cpus` to `500_000_000` results in the container being allocated"
    "50% of a single CPU, while `2_000_000_000` results in the container being allocated 2 CPUs."
    nano_cpus: u64 => "HostConfig.NanoCpus");

    /// CPU quota in units of CPUs. This is a wrapper around `nano_cpus` to do the unit conversion.
    ///
    /// See [`nano_cpus`](#method.nano_cpus).
    pub fn cpus(&mut self, cpus: f64) -> &mut Self {
        self.nano_cpus((1_000_000_000.0 * cpus) as u64)
    }

    impl_field!(
    "Sets an integer value representing the container's relative CPU weight versus other"
    "containers."
    cpu_shares: u32 => "HostConfig.CpuShares");

    impl_map_field!(labels: L => "Labels");

    /// Whether to attach to `stdin`.
    pub fn attach_stdin(&mut self, attach: bool) -> &mut Self {
        self.params.insert("AttachStdin", json!(attach));
        self.params.insert("OpenStdin", json!(attach));
        self
    }

    impl_field!(
    "Whether to attach to `stdout`."
    attach_stdout: bool => "AttachStdout");

    impl_field!(
    "Whether to attach to `stderr`."
    attach_stderr: bool => "AttachStderr");

    impl_field!(
    "Whether standard streams should be attached to a TTY."
    tty: bool => "Tty");

    impl_vec_field!(extra_hosts: H => "HostConfig.ExtraHosts");

    impl_vec_field!(volumes_from: V => "HostConfig.VolumesFrom");

    impl_str_field!(network_mode: M => "HostConfig.NetworkMode");

    impl_vec_field!(env: E => "Env");

    impl_vec_field!(cmd: C => "Cmd");

    impl_vec_field!(entrypoint: E => "Entrypoint");

    impl_vec_field!(capabilities: C => "HostConfig.CapAdd");

    pub fn devices(&mut self, devices: Vec<Labels>) -> &mut Self {
        self.params.insert("HostConfig.Devices", json!(devices));
        self
    }

    impl_str_field!(log_driver: L => "HostConfig.LogConfig.Type");

    pub fn restart_policy(&mut self, name: &str, maximum_retry_count: u64) -> &mut Self {
        self.params
            .insert("HostConfig.RestartPolicy.Name", json!(name));
        if name == "on-failure" {
            self.params.insert(
                "HostConfig.RestartPolicy.MaximumRetryCount",
                json!(maximum_retry_count),
            );
        }
        self
    }

    impl_field!(auto_remove: bool => "HostConfig.AutoRemove");

    impl_str_field!(
    "Signal to stop a container as a string. Default is \"SIGTERM\""
    stop_signal: S => "StopSignal");

    impl_field!(
    "Signal to stop a container as an integer. Default is 15 (SIGTERM)."
    stop_signal_num: u64 => "StopSignal");

    impl_field!(
    "Timeout to stop a container. Only seconds are counted. Default is 10s"
    stop_timeout: Duration => "StopTimeout");

    impl_str_field!(userns_mode: M => "HostConfig.UsernsMode");

    impl_field!(privileged: bool => "HostConfig.Privileged");

    impl_str_field!(user: U => "User");

    pub fn build(&self) -> ContainerOpts {
        ContainerOpts {
            name: self.name.clone(),
            params: self.params.clone(),
        }
    }
}

impl_url_opts_builder!(Logs);

impl LogsOptsBuilder {
    impl_url_bool_field!(follow => "follow");

    impl_url_bool_field!(stdout => "stdout");

    impl_url_bool_field!(stderr => "stderr");

    impl_url_bool_field!(timestamps => "timestamps");

    impl_url_str_field!(tail: N => "tail");

    #[cfg(feature = "chrono")]
    pub fn since<Tz>(&mut self, timestamp: &chrono::DateTime<Tz>) -> &mut Self
    where
        Tz: chrono::TimeZone,
    {
        self.params
            .insert("since", timestamp.timestamp().to_string());
        self
    }

    #[cfg(not(feature = "chrono"))]
    pub fn since(&mut self, timestamp: i64) -> &mut Self {
        self.params.insert("since", timestamp.to_string());
        self
    }
}

impl_url_opts_builder!(RmContainer);

impl RmContainerOptsBuilder {
    impl_url_bool_field!(force => "force");

    impl_url_bool_field!(volumes => "v");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_options_simple() {
        let builder = ContainerOptsBuilder::new("test_image");
        let options = builder.build();

        assert_eq!(
            r#"{"HostConfig":{},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
    }

    #[test]
    fn container_options_env() {
        let options = ContainerOptsBuilder::new("test_image")
            .env(vec!["foo", "bar"])
            .build();

        assert_eq!(
            r#"{"Env":["foo","bar"],"HostConfig":{},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
    }

    #[test]
    fn container_options_env_dynamic() {
        let env: Vec<String> = ["foo", "bar", "baz"]
            .iter()
            .map(|s| String::from(*s))
            .collect();

        let options = ContainerOptsBuilder::new("test_image").env(&env).build();

        assert_eq!(
            r#"{"Env":["foo","bar","baz"],"HostConfig":{},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
    }

    #[test]
    fn container_options_user() {
        let options = ContainerOptsBuilder::new("test_image")
            .user("alice")
            .build();

        assert_eq!(
            r#"{"HostConfig":{},"Image":"test_image","User":"alice"}"#,
            options.serialize().unwrap()
        );
    }

    #[test]
    fn container_options_host_config() {
        let options = ContainerOptsBuilder::new("test_image")
            .network_mode("host")
            .auto_remove(true)
            .privileged(true)
            .build();

        assert_eq!(
            r#"{"HostConfig":{"AutoRemove":true,"NetworkMode":"host","Privileged":true},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
    }

    #[test]
    fn container_options_expose() {
        let options = ContainerOptsBuilder::new("test_image")
            .expose(80, "tcp", 8080)
            .build();
        assert_eq!(
            r#"{"ExposedPorts":{"80/tcp":{}},"HostConfig":{"PortBindings":{"80/tcp":[{"HostPort":"8080"}]}},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
        // try exposing two
        let options = ContainerOptsBuilder::new("test_image")
            .expose(80, "tcp", 8080)
            .expose(81, "tcp", 8081)
            .build();
        assert_eq!(
            r#"{"ExposedPorts":{"80/tcp":{},"81/tcp":{}},"HostConfig":{"PortBindings":{"80/tcp":[{"HostPort":"8080"}],"81/tcp":[{"HostPort":"8081"}]}},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
    }

    #[test]
    fn container_options_publish() {
        let options = ContainerOptsBuilder::new("test_image")
            .publish(80, "tcp")
            .build();
        assert_eq!(
            r#"{"ExposedPorts":{"80/tcp":{}},"HostConfig":{},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
        // try exposing two
        let options = ContainerOptsBuilder::new("test_image")
            .publish(80, "tcp")
            .publish(81, "tcp")
            .build();
        assert_eq!(
            r#"{"ExposedPorts":{"80/tcp":{},"81/tcp":{}},"HostConfig":{},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
    }

    /// Test container option PublishAllPorts
    #[test]
    fn container_options_publish_all_ports() {
        let options = ContainerOptsBuilder::new("test_image")
            .publish_all_ports()
            .build();

        assert_eq!(
            r#"{"HostConfig":{"PublishAllPorts":true},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
    }

    /// Test container Opts that are nested 3 levels deep.
    #[test]
    fn container_options_nested() {
        let options = ContainerOptsBuilder::new("test_image")
            .log_driver("fluentd")
            .build();

        assert_eq!(
            r#"{"HostConfig":{"LogConfig":{"Type":"fluentd"}},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
    }

    /// Test the restart policy settings
    #[test]
    fn container_options_restart_policy() {
        let mut options = ContainerOptsBuilder::new("test_image")
            .restart_policy("on-failure", 10)
            .build();

        assert_eq!(
            r#"{"HostConfig":{"RestartPolicy":{"MaximumRetryCount":10,"Name":"on-failure"}},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );

        options = ContainerOptsBuilder::new("test_image")
            .restart_policy("always", 0)
            .build();

        assert_eq!(
            r#"{"HostConfig":{"RestartPolicy":{"Name":"always"}},"Image":"test_image"}"#,
            options.serialize().unwrap()
        );
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn logs_options() {
        let timestamp = chrono::NaiveDateTime::from_timestamp(2_147_483_647, 0);
        let since = chrono::DateTime::<chrono::Utc>::from_utc(timestamp, chrono::Utc);

        let options = LogsOptsBuilder::default()
            .follow(true)
            .stdout(true)
            .stderr(true)
            .timestamps(true)
            .tail("all")
            .since(&since)
            .build();

        let serialized = options.serialize().unwrap();

        assert!(serialized.contains("follow=true"));
        assert!(serialized.contains("stdout=true"));
        assert!(serialized.contains("stderr=true"));
        assert!(serialized.contains("timestamps=true"));
        assert!(serialized.contains("tail=all"));
        assert!(serialized.contains("since=2147483647"));
    }

    #[cfg(not(feature = "chrono"))]
    #[test]
    fn logs_Opts() {
        let options = LogsOptsBuilder::default()
            .follow(true)
            .stdout(true)
            .stderr(true)
            .timestamps(true)
            .tail("all")
            .since(2_147_483_647)
            .build();

        let serialized = options.serialize().unwrap();

        assert!(serialized.contains("follow=true"));
        assert!(serialized.contains("stdout=true"));
        assert!(serialized.contains("stderr=true"));
        assert!(serialized.contains("timestamps=true"));
        assert!(serialized.contains("tail=all"));
        assert!(serialized.contains("since=2147483647"));
    }
}
