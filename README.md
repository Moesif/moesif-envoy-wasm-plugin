# Moesif Envoy WASM Plugin

The Moesif Envoy WebAssembly plugin captures API traffic from [Envoy Service Proxy](https://www.envoyproxy.io/) and logs it to [Moesif API Analytics and Monetization](https://www.moesif.com) platform. Supports projects built on Envoy such as Solo.io Gloo Gateway, Istio, and others. This plugin leverages an asynchronous design and doesn’t add any latency to your API calls.

- Envoy is a L7 proxy and communication bus.
- Moesif is an API analytics and monetization platform.

[Source Code on GitHub](https://github.com/Moesif/moesif-envoy-wasm-plugin)

> Moesif has both an Envoy plugin [built in WASM](https://www.moesif.com/docs/server-integration/envoy-wasm-plugin/) and one [built in Lua](https://www.moesif.com/docs/server-integration/envoy-lua-plugin/). For most projects including Gloo and Istio, Moesif recommends the WASM plugin for full compatibility.

## How to Install

### 1. Download the Plugin

The `moesif_envoy_wasm_plugin.wasm` file can be downloaded directly from the GitHub releases page. To do so:

1. Navigate to the [GitHub release page](https://github.com/Moesif/moesif-envoy-wasm-plugin/releases).
2. Find the latest release and download the `moesif_envoy_wasm_plugin.wasm` file from the assets section.

### 2. Load the Plugin into your Envoy Proxy

1. Transfer the downloaded `moesif_envoy_wasm_plugin.wasm` file to the Envoy proxy server.
2. Place the wasm file in an appropriate directory (for example, `/etc/envoy/proxy-wasm-plugins/`).
3. Ensure the Envoy proxy has read access to the wasm file.

### 3. Configure Envoy

Update your Envoy configuration (`envoy.yaml`) to use the Moesif Envoy plugin. Add the `http_filters` and `clusters` sections as shown in the provided code snippets.

Remember to replace `<YOUR APPLICATION ID HERE>` with your actual Moesif Application Id. Your Moesif Application Id can be found in the [_Moesif Portal_](https://www.moesif.com/). After signing up for a Moesif account, your Moesif Application Id will be displayed during the onboarding steps.

The `upstream` config defaults to 'moesif_api' and is optional, but if you use something else in the clusters.name field, you will need to explicitly include the `upstream` config value to match.

Also, remember to update the filename path in the `vm_config` section to match the location where you placed the `moesif_envoy_wasm_plugin.wasm` file.

```yaml
http_filters:
# ... other filters ...
- name: envoy.filters.http.wasm
  typed_config:
    "@type": type.googleapis.com/envoy.extensions.filters.http.wasm.v3.Wasm
    config:
      name: "moesif_api"
      root_id: "moesif_api_root_id"
      configuration:
        "@type": "type.googleapis.com/google.protobuf.StringValue"
        value: |
          {
            "moesif_application_id":"<YOUR APPLICATION ID HERE>", 
            "user_id_header":"X-User-Example-Header",
            "company_id_header":"X-Company-Example-Header",
            "upstream": "moesif_api"
          }
      vm_config:
        vm_id: "moesif_api_vm"
        code:
          local:
            filename: "/etc/envoy/proxy-wasm-plugins/moesif_envoy_wasm_plugin.wasm"
# ... other filters ending with router
- name: envoy.filters.http.router
  typed_config:
    "@type": type.googleapis.com/envoy.extensions.filters.http.router.v3.Router
clusters:
# ... other clusters ...
- name: moesif_api
  type: strict_dns
  load_assignment:
    cluster_name: moesif_api
    endpoints:
    - lb_endpoints:
      - endpoint:
          address:
            socket_address:
              address: api.moesif.net
              port_value: 443
  transport_socket:
    name: envoy.transport_sockets.tls
    typed_config:
      "@type": type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.UpstreamTlsContext
```

### 4. Restart Envoy

After saving the updated configuration file, restart Envoy to apply the changes. Check Envoy's log to ensure that there are no errors during startup.

### 5. Test

Make a few API calls that pass through the Envoy proxy. These calls should now be logged to your Moesif account.

## How to Use

###  Capturing API traffic
The Moesif Envoy plugin captures API traffic from Envoy and logs it to Moesif automatically when Envoy routes traffic through the plugin.  Envoy has detailed and robust configuration options for traffic and plugin routing to apply the Moesif plugin to only some traffic or to all traffic. For more information, please refer to the [Envoy Request Lifecycle Guide](https://www.envoyproxy.io/docs/envoy/latest/intro/life_of_a_request#configuration).

###  Identifying users and companies

This plugin will automatically identify API users so you can associate API traffic to web traffic and create cross-platform funnel reports of your customer journey. The plugin currently supports reading request headers to identify users and companies automatically from events.

- If the `user_id_header` or `company_id_header` configuration option is set, the named request header will be read from each request and it's value will be included in the Moesif event model as the `user_id` or `company_id` field respectively.
2. You can associate API users to companies for tracking account-level usage. This can be done either with the company header above or through the Moesif [update user API](https://www.moesif.com/docs/api#update-a-user) to set a `company_id` for a user. Moesif will associate the API calls automatically.

## Configuration Options

These configuration options are specified as JSON in the `configuration` section of the `http_filters` in your `envoy.yaml` file.

| Option                 | Type    | Default                 | Description                                                                                                                             |
|------------------------|---------|-------------------------|-----------------------------------------------------------------------------------------------------------------------------------------|
| `moesif_application_id`| String  | None                    | **Required.** Your Moesif Application Id. Can be found within the Moesif Portal.                                                        |
| `user_id_header`       | String  | None                    | Optional. The header key for User Id. If provided, the corresponding header value is used as the User Id in Moesif event models.        |
| `company_id_header`    | String  | None                    | Optional. The header key for Company Id. If provided, the corresponding header value is used as the Company Id in Moesif event models.  |
| `batch_max_size`       | Integer | 100                     | Optional. The maximum batch size of events to be sent to Moesif.                                                                       |
| `batch_max_wait`       | Integer | 2000                    | Optional. The maximum wait time in milliseconds before a batch is sent to Moesif, regardless of the batch size.                              |
| `upstream`             | String  | "moesif_api"            | Optional. The upstream cluster that points to Moesif's API.                                                                            |

### Example

```yaml
configuration:
  "@type": "type.googleapis.com/google.protobuf.StringValue"
  value: |
    {
      "moesif_application_id":"<YOUR APPLICATION ID HERE>", 
      "user_id_header":"X-User-Example-Header",
      "company_id_header":"X-Company-Example-Header",
      "batch_max_size": 100,
      "batch_max_wait": 5,
      "upstream": "example_custom_envoy_cluster_naming_scheme_moesif"
    }
```

### Updating the Configuration

Updating the envoy.yaml configuration file in the example above and restarting is sufficient to update your Moesif WASM Plugin configuration. Envoy has a diversity of configuration mechanisms and supports hot reloading of configuration. For more information, please refer to the [Envoy Configuration Documentation](https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/operations/dynamic_configuration).

## Examples

### Envoy Docker Compose

If you're using Docker, you can use the provided `docker-compose.yaml` to easily set up your environment. The Docker Compose file includes three services:

- `rust-builder`: This service builds the wasm binary from the Rust source code.
- `envoy`: This service runs the Envoy proxy with the Moesif Envoy plugin.
- `echo`: This is a simple echo server for testing purposes.

### Steps

1. Ensure Docker and Docker Compose are installed on your system.
2. Clone the project repository.

    ```bash
    git clone https://github.com/Moesif/moesif-envoy-wasm-plugin.git
    cd moesif-envoy-plugin/examples/envoy
    ```

3. Build and start the services using Docker Compose.

    ```bash
    docker-compose up --build
    ```

4. Now, you can send requests to `http://localhost:10000`. The Envoy proxy listens on this port, forwards the requests to the echo server, and logs the requests and responses to Moesif.

5. You can view the Envoy logs with the following command:

    ```bash
    docker-compose logs envoy
    ```

Remember to replace `<YOUR APPLICATION ID HERE>` in `envoy.yaml` file with your actual Moesif Application Id. Your Moesif Application Id can be found in the [_Moesif Portal_](https://www.moesif.com/).

### Moesif Istio WASM Plugin Example

This README describes how to install and configure the Moesif Istio WASM Plugin with Minikube, using the Kubernetes resources provided.

### Prerequisites

You'll need access to a Kubernetes cluster into which to install the Moesif Istio WASM Plugin. This example uses Minikube to provide everything you need, but you can use any Kubernetes cluster:
- [Istioctl](https://istio.io/latest/docs/setup/getting-started/#download) (v1.10.0 or later)
- [Minikube](https://minikube.sigs.k8s.io/docs/start/) (v1.27 or later)
- [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/) (v1.25 or later)

Start Minikube giving it enough resources:

```bash
minikube start --cpus 4 --memory 8192
```

### Istio Installation

We're going to use the Istio CLI tool for the installation.

1. Install the Istio base chart which includes cluster-wide resources used by the Istio operator:

```bash
istioctl install --set profile=demo -y
```

2. Check the status of the Istio system pods:

```bash
kubectl get pods -n istio-system
```

### Deploy the Moesif Istio WASM Plugin Example

1. Navigate to the `examples/istio` directory in this repository:

2. Apply the resources to your Minikube cluster.  This will create the echo service, Istio inbound configuration to route traffic to the echo service, Istio outbound configuration to allow Istio to contact Moesif's API, and the Moesif WASM plugin itself:

```bash
kubectl apply -f echo-service.yaml \
 -f istio-echo-inbound.yaml \
 -f istio-moesif-outbound.yaml \
 -f moesif-wasm-plugin.yaml
```

3. Verify that the echo service is running:

```bash
kubectl get pods -n default
```

You should see the `echo` pod in the Running status.

4. Test the echo service:

```bash
kubectl port-forward svc/echo 8080:80
```

Now you can send a request to the echo service running inside the cluster from the host machine:

```bash
curl http://localhost:8080/echo
```

You should get a response: `Hello from echo service` validating the service is running.

### Moesif WASM Plugin Configuration

The WasmPlugin YAML definition in `moesif-wasm-plugin.yaml` configures the Moesif Istio WASM Plugin.  The pluginConfig section of the YAML definition is shown below:

```yaml
    moesif_application_id: <YOUR MOESIF APPLICATION ID>
    upstream: outbound|443||api.moesif.net
```

This configuration allows the plugin to capture and log the requests and responses flowing through the Istio service mesh. To use the plugin, you need a Moesif application id, which is set in the `moesif_application_id` field in the plugin configuration. You can get this from your Moesif dashboard.

Remember to replace the `moesif_application_id` and `upstream` values in with your actual values.  The upstream string value is the cluster name that points to Moesif's API in the Istio outbound configuration.  `debug` is set to `true` to enable debug logging for the example, but this should be set to `false` in production.

### Accessing the echo service via Istio Ingress Gateway

Next, access the echo service via Istio Ingress Gateway, you first need to determine the ingress IP and ports:

- For Minikube:

```bash
export INGRESS_HOST=$(minikube ip)
export INGRESS_PORT=$(kubectl -n istio-system get service istio-ingressgateway -o jsonpath='{.spec.ports[?(@.name=="http2")].nodePort}')
```

Now, you can send a request to the echo service through the Istio Ingress Gateway and the Moesif WASM Plugin:

```bash
curl -sS "http://${INGRESS_HOST}:${INGRESS_PORT}/echo"
```

The response should be `Hello from echo service`.

### Finished, Check Logs

After the configuration is applied, you can check the events in your https://moesif.com account to see the plugin in action.

## Other Integrations

To view more documentation on integration options, please visit __[the Integration Options Documentation](https://www.moesif.com/docs/getting-started/integration-options/).__
