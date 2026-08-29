import { ReflexClient } from "./client";
import { ActionGraph, ActionNode } from "./types";

export interface LLMHealerContext {
  graphId: string;
  version: number;
  failedStepId: string;
  htmlSnippet: string;
}

export interface ReflexSessionOptions {
  endpoint?: string;
  approvalCallback?: (node: ActionNode) => Promise<boolean> | boolean;
  llmHealer?: (ctx: LLMHealerContext) => Promise<string> | string;
}

export interface ExecuteOptions {
  valueSlots?: Record<string, string>;
}

export interface ExecuteResult {
  status: "success" | "miss" | "failed";
  graphId?: string;
  version?: number;
  completedSteps?: string[];
  failedStep?: string;
  elapsedMs?: number;
  message?: string;
}

export class ReflexSession {
  private page: any;
  private client: ReflexClient;
  private approvalCallback?: (node: ActionNode) => Promise<boolean> | boolean;
  private llmHealer?: (ctx: LLMHealerContext) => Promise<string> | string;

  constructor(page: any, options: ReflexSessionOptions = {}) {
    this.page = page;
    this.client = new ReflexClient(options.endpoint || "http://127.0.0.1:9199");
    this.approvalCallback = options.approvalCallback;
    this.llmHealer = options.llmHealer;
  }

  async execute(options: ExecuteOptions = {}): Promise<ExecuteResult> {
    const slots = options.valueSlots || {};
    const html: string = await this.page.content();
    let domain: string | undefined;

    try {
      const url = new URL(this.page.url());
      domain = url.host;
    } catch {
      // Ignore if invalid URL
    }

    const inspectRes = await this.client.inspect(html, domain);
    let graph = inspectRes.graph;

    if (!graph || inspectRes.status === "miss") {
      return {
        status: "miss",
        message: "No cached action graph found for this page skeleton.",
      };
    }

    const startTime = performance.now();
    const completedSteps: string[] = [];

    for (const node of graph.nodes) {
      // 1. Safety Gate
      if (node.requires_approval || node.safety_level === "mutating_write") {
        if (this.approvalCallback) {
          const approved = await this.approvalCallback(node);
          if (!approved) {
            throw new Error(`Execution blocked: Step '${node.step_id}' rejected by approval gate.`);
          }
        } else {
          throw new Error(`Execution blocked: Step '${node.step_id}' requires human approval.`);
        }
      }

      // 2. Resolve Slot Value
      let resolvedValue: string | undefined;
      if (node.value_slot) {
        const slotKey = node.value_slot.replace(/^\$/, "");
        resolvedValue = slots[slotKey] ?? slots[node.value_slot] ?? "";
      }

      // 3. Execute with Fallback & Self-Healing
      let success = await this.executeNode(node, resolvedValue);
      if (!success) {
        if (this.llmHealer) {
          const ctx: LLMHealerContext = {
            graphId: graph.graph_id,
            version: graph.version,
            failedStepId: node.step_id,
            htmlSnippet: html.slice(0, 3000),
          };
          const newSelector = await this.llmHealer(ctx);
          if (newSelector) {
            const healedGraph = await this.client.heal(
              graph,
              node.step_id,
              newSelector,
              inspectRes.current_skeleton_hash
            );
            graph = healedGraph;
            node.target.primary = newSelector;
            success = await this.executeNode(node, resolvedValue);
          }
        }

        if (!success) {
          return {
            status: "failed",
            failedStep: node.step_id,
            completedSteps,
          };
        }
      }

      completedSteps.push(node.step_id);
    }

    const elapsedMs = performance.now() - startTime;
    return {
      status: "success",
      graphId: graph.graph_id,
      version: graph.version,
      completedSteps,
      elapsedMs: Math.round(elapsedMs * 100) / 100,
    };
  }

  private async executeNode(node: ActionNode, value?: string): Promise<boolean> {
    const selectors = [node.target.primary, ...(node.target.fallbacks || [])];
    for (const sel of selectors) {
      try {
        if (node.action_type === "click") {
          await this.page.click(sel, { timeout: 1000 });
          return true;
        } else if (node.action_type === "type") {
          await this.page.fill(sel, value || "", { timeout: 1000 });
          return true;
        } else if (node.action_type === "wait_for") {
          await this.page.waitForSelector(sel, { timeout: 1000 });
          return true;
        }
      } catch {
        continue;
      }
    }
    return false;
  }
}
