# ADR 0006: Browser Access Through Edge Routing

Status: accepted for MVP preview.

## Context

The Password Vault MVP runs in the home Kubernetes cluster. Kubernetes Services may expose
cluster/LXD load-balancer addresses, while normal browser clients such as a MacBook are expected to
connect through the mini-PC home-LAN address.

This distinction caused confusion: a Kubernetes `LoadBalancer` address can be correct from the
cluster/LXD routing point of view while still being unreachable from a LAN-only browser client.

## Decision

For MVP preview browser access, use the mini-PC edge route as the user-facing access path:

```text
Password Vault: https://<mini-pc-lan-ip>:11443/
Grafana:        https://<mini-pc-lan-ip>:3000/
Argo CD:        https://<mini-pc-lan-ip>:9443/
```

Do not document Kubernetes/LXD `LoadBalancer`, Pod, or ClusterIP addresses as default browser URLs
for a MacBook or other LAN-only client.

Use Kubernetes/LXD service addresses only for cluster-side diagnostics or clients that explicitly
have a route or VPN into that network.

## Rationale

Kubernetes documents `LoadBalancer` as a Service type that exposes a Service through an external
load balancer implementation. In this home cluster, that implementation publishes addresses in the
LXD/Kubernetes network, not necessarily on the same route as a normal LAN client.

The mini-PC host already runs edge NGINX:

- HTTP reverse proxying for Grafana and Password Vault;
- TCP stream proxying for Argo CD HTTPS;
- TLS termination or pass-through behavior appropriate to each route.

This makes the mini-PC LAN address the stable human/browser access point while keeping Kubernetes
Service addresses as internal routing details.

## Consequences

- Browser docs and support responses must show the mini-PC LAN address plus the edge-published
  port.
- Runtime diagnostics should check both layers when access fails:
  - edge listener on the mini-PC;
  - NGINX upstream to the Kubernetes/LXD Service;
  - Argo CD application health;
  - client-side LAN/VPN/firewall/certificate behavior.
- The internal black-box `/readyz` probe proves Kubernetes service reachability from observability,
  not MacBook browser reachability.
- A scheduled external synthetic check should eventually exercise the edge route, not only the
  in-cluster Service.
- If the platform later moves to Ingress, Gateway API, VPN-only access, or public DNS/TLS, this ADR
  must be replaced or amended.

## Rejected Options

- Publish Kubernetes/LXD `LoadBalancer` addresses as default user URLs: rejected because they are
  not necessarily routable from normal LAN clients.
- Expose PostgreSQL or internal metrics through the browser edge route: rejected. PostgreSQL must
  remain private, and edge `/metrics` should stay blocked.
- Treat local mini-PC `curl` success as proof of MacBook access: rejected. It proves the edge
  listener and upstream path from the host, but not the client network path or certificate handling.

## Sources

- Kubernetes Service documentation:
  <https://kubernetes.io/docs/concepts/services-networking/service/>
- NGINX reverse proxy documentation:
  <https://docs.nginx.com/nginx/admin-guide/web-server/reverse-proxy/>
- NGINX TCP/UDP load balancing documentation:
  <https://docs.nginx.com/nginx/admin-guide/load-balancer/tcp-udp-load-balancer/>
