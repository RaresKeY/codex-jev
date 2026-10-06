# Jev routing provenance

- Upstream: https://github.com/openai/codex, rust-v0.160.0,
  a956835d020762cb2b570053af06f643a11c0ecc; Apache 2.0. Full upstream source,
  license and notices remain in this fork; source was inspected where modified.
- User-owned codex-webui-2: backend/app/task_router.py. Adapted payload construction,
  answer validation and external credential parsing into jev/route.py. Browser,
  persistence, server and frontend code were not imported.
- User-owned jev-task-router: routing_policy.json, V5 dated 2026-10-05. Public,
  generic routing policy snapshot is used as jev/policy.json. Experiments, session
  data, reports, vendor SDK code and credentials were not imported.
- The user explicitly requested public open source publication of this adaptation.
  These first-party fork additions are distributed under the fork's Apache 2.0
  license; no separately licensed vendor client was copied.
- Provider dependency: Typesafe/Jev HTTPS API; not vendored, not an OpenAI model.
  Users supply their own provider credentials; no credential is included.
