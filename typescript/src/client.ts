import { ActionGraph, GraphHistoryEntry, HealResponse, InspectResponse } from "./types";

export interface ReflexClientOptions {
  endpoint?: string;
  apiKey?: string;
  tenantId?: string;
}

export class ReflexClient {
  private endpoint: string;
  private apiKey?: string;
  private tenantId?: string;

  constructor(optionsOrEndpoint: string | ReflexClientOptions = "http://127.0.0.1:9199") {
    if (typeof optionsOrEndpoint === "string") {
      this.endpoint = optionsOrEndpoint.replace(/\/$/, "");
    } else {
      this.endpoint = (optionsOrEndpoint.endpoint || "http://127.0.0.1:9199").replace(/\/$/, "");
      this.apiKey = optionsOrEndpoint.apiKey;
      this.tenantId = optionsOrEndpoint.tenantId;
    }
  }

  private headers(customHeaders?: Record<string, string>): Record<string, string> {
    const h: Record<string, string> = {
      "Content-Type": "application/json",
      ...customHeaders,
    };
    if (this.apiKey) {
      h["Authorization"] = `Bearer ${this.apiKey}`;
    }
    if (this.tenantId) {
      h["X-Tenant-ID"] = this.tenantId;
    }
    return h;
  }

  async health(): Promise<{ status: string; version: string }> {
    const res = await fetch(`${this.endpoint}/api/v1/health`);
    if (!res.ok) throw new Error(`Health check failed: ${res.statusText}`);
    return res.json() as Promise<{ status: string; version: string }>;
  }

  async hashHtml(html: string): Promise<string> {
    const res = await fetch(`${this.endpoint}/api/v1/hash`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify({ html }),
    });
    if (!res.ok) throw new Error(`Hash failed: ${res.statusText}`);
    const data = (await res.json()) as { skeleton_hash: string };
    return data.skeleton_hash;
  }

  async inspect(html: string, domain?: string): Promise<InspectResponse> {
    const res = await fetch(`${this.endpoint}/api/v1/inspect`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify({ html, domain }),
    });
    if (!res.ok) throw new Error(`Inspect failed: ${res.statusText}`);
    return res.json() as Promise<InspectResponse>;
  }

  async record(graph: ActionGraph): Promise<{ status: string; graph_id: string; version: number }> {
    const res = await fetch(`${this.endpoint}/api/v1/record`, {
      method: "POST",
      headers: this.headers(),
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
      headers: this.headers(),
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

  async getHistory(graphId: string): Promise<GraphHistoryEntry[]> {
    const res = await fetch(`${this.endpoint}/api/v1/graphs/${graphId}/history`, {
      method: "GET",
      headers: this.headers(),
    });
    if (!res.ok) throw new Error(`GetHistory failed: ${res.statusText}`);
    const data = (await res.json()) as { graph_id: string; history: GraphHistoryEntry[] };
    return data.history;
  }

  async rollback(graphId: string, version: number): Promise<ActionGraph> {
    const res = await fetch(`${this.endpoint}/api/v1/graphs/${graphId}/rollback`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify({ version }),
    });
    if (!res.ok) throw new Error(`Rollback failed: ${res.statusText}`);
    const data = (await res.json()) as { status: string; graph: ActionGraph };
    return data.graph;
  }

  async getStats(): Promise<any> {
    const res = await fetch(`${this.endpoint}/api/v1/stats`, {
      method: "GET",
      headers: this.headers(),
    });
    if (!res.ok) throw new Error(`GetStats failed: ${res.statusText}`);
    return res.json();
  }
}