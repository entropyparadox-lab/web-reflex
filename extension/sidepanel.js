const DAEMON_URL = "http://127.0.0.1:9199";

const cacheStatusEl = document.getElementById("cacheStatus");
const skeletonHashEl = document.getElementById("skeletonHash");
const recipeInfoEl = document.getElementById("recipeInfo");
const graphIdEl = document.getElementById("graphId");
const graphVerEl = document.getElementById("graphVer");
const stepsListEl = document.getElementById("stepsList");
const btnReplay = document.getElementById("btnReplay");
const btnInspect = document.getElementById("btnInspect");

let currentGraph = null;

async function inspectActiveTab() {
  cacheStatusEl.className = "status-badge status-miss";
  cacheStatusEl.textContent = "Inspecting...";
  btnReplay.style.display = "none";
  recipeInfoEl.style.display = "none";

  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab || !tab.id) {
    cacheStatusEl.textContent = "No Tab";
    return;
  }

  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId: tab.id },
      func: () => document.documentElement.outerHTML,
    });

    const html = results[0]?.result || "";
    const urlObj = new URL(tab.url || "http://localhost");
    const domain = urlObj.host;

    // Call Daemon
    const resp = await fetch(`${DAEMON_URL}/api/v1/inspect`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ html, domain }),
    });

    if (!resp.ok) throw new Error("Daemon unavailable");
    const data = await resp.json();

    if (data.status === "hit") {
      cacheStatusEl.className = "status-badge status-hit";
      cacheStatusEl.textContent = "🎯 Cache Hit";
      currentGraph = data.graph;
      skeletonHashEl.textContent = data.graph.skeleton_hash;
      graphIdEl.textContent = data.graph.graph_id;
      graphVerEl.textContent = `v${data.graph.version}`;

      stepsListEl.innerHTML = "";
      for (const node of data.graph.nodes) {
        const li = document.createElement("li");
        li.className = "step-item";
        li.textContent = `${node.action_type.toUpperCase()}: ${node.target.primary}`;
        stepsListEl.appendChild(li);
      }

      recipeInfoEl.style.display = "block";
      btnReplay.style.display = "block";
    } else if (data.status === "candidate") {
      cacheStatusEl.className = "status-badge status-candidate";
      cacheStatusEl.textContent = "🔍 Candidate";
      skeletonHashEl.textContent = data.current_skeleton_hash;
      recipeInfoEl.style.display = "none";
    } else {
      cacheStatusEl.className = "status-badge status-miss";
      cacheStatusEl.textContent = "⚡ Cache Miss";
      skeletonHashEl.textContent = data.skeleton_hash || "unknown";
      recipeInfoEl.style.display = "none";
    }
  } catch (err) {
    cacheStatusEl.className = "status-badge status-miss";
    cacheStatusEl.textContent = "Daemon Offline";
    skeletonHashEl.textContent = "Start: ./web-reflex serve";
    console.error(err);
  }
}

btnInspect.addEventListener("click", inspectActiveTab);
btnReplay.addEventListener("click", async () => {
  if (!currentGraph) return;
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab || !tab.id) return;

  await chrome.scripting.executeScript({
    target: { tabId: tab.id },
    func: (graph) => {
      for (const node of graph.nodes) {
        const el = document.querySelector(node.target.primary);
        if (el) {
          if (node.action_type === "click") el.click();
          else if (node.action_type === "type") el.value = "SAMPLE_INPUT";
        }
      }
    },
    args: [currentGraph],
  });
  alert("⚡ Fast-Path Replay Executed!");
});

// Auto inspect on load
document.addEventListener("DOMContentLoaded", inspectActiveTab);
