import { ActionGraph, HealResponse, InspectResponse } from "./types";

export class ReflexClient {
  private endpoint: string;

  constructor(endpoint = "http://127.0.0.1:9199") {
    this.endpoint = endpoint.replace(/\/$/, "");
  }

  async health(): Promise<{ status: string; version: string }> {
    const res = await fetch(`${this.endpoint}/api/v1/health`);
    if (!res.ok) throw new Error(`Health check failed: ${res.statusText}`);
    return res.json() as Promise<{ status: string; version: string }>;
  }

  async hashHtml(html: string): Promise<string> {
    const res = await fetch(`${this.endpoint}/api/v1/hash`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ html }),
    });
    if (!res.ok) throw new Error(`Hash failed: ${res.statusText}`);
    const data = (await res.json()) as { skeleton_hash: string };
    return data.skeleton_hash;
  }

  async inspect(html: string, domain?: string): Promise<InspectResponse> {
    const res = await fetch(`${this.endpoint}/api/v1/inspect`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ html, domain }),
    });
    if (!res.ok) throw new Error(`Inspect failed: ${res.statusText}`);
    return res.json() as Promise<InspectResponse>;
  }

  async record(graph: ActionGraph): Promise<{ status: string; graph_id: string; version: number }> {
    const res = await fetch(`${this.endpoint}/api/v1/record`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ graph }),
    });
    if (!res.ok) throw new Error(`Record failed: ${res.statusText}`);
    return res.json() as Promise<{ status: string; graph_id: string; version: number }>;
  }

  async heal(
    graph: ActionGraph,
    stepId: string,
    newPrimarySelector: string,
    newSkeletonHash?: string
  ): Promise<ActionGraph> {
    const res = await fetch(`${this.endpoint}/api/v1/heal`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        graph,
        step_id: stepId,
        new_primary_selector: newPrimarySelector,
        new_skeleton_hash: newSkeletonHash,
      }),
    });
    if (!res.ok) throw new Error(`Heal failed: ${res.statusText}`);
    const data = (await res.json()) as HealResponse;
    return data.graph;
  }
}
