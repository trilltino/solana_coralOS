import { useEffect, useState } from "react";
import { invoke, IS_TAURI, listenEvent } from "./transport";

interface AgentState {
  is_running: boolean;
  actions: AgentAction[];
  rpc_endpoint: string;
  network: string;
  strategy: string;
}

interface AgentAction {
  timestamp: string;
  action_type: string;
  details: string;
  tx_signature: string | null;
  slot: number | null;
  latency_ms: number;
}

interface AgentMeta {
  role: string;
  created_at: string;
  tags: string[];
}

interface AgentMessage {
  id: string;
  from: string;
  to: string | null;
  msg_type: string;
  payload: string;
  timestamp: string;
}

interface WorkflowStep {
  id: string;
  name: string;
  description: string;
  status: string;
  assigned_to: string | null;
  dependencies: string[];
  result: string | null;
  started_at: string | null;
  completed_at: string | null;
  timeout_secs: number | null;
}

interface Workflow {
  id: string;
  name: string;
  description: string;
  status: string;
  steps: WorkflowStep[];
  current_step: number;
  created_at: string;
  updated_at: string;
  created_by: string;
  assigned_agents: string[];
  priority: number;
  tags: string[];
}

interface SharedStateEntry {
  value: unknown;
  last_modified: string;
  modified_by: string;
  version: number;
}

interface StateChange {
  key: string;
  old_value: unknown | null;
  new_value: unknown;
  timestamp: string;
  changed_by: string;
}

interface CoralSession {
  id: string;
  namespace: string;
  status: string;
  agentCount?: number;
  paymentSessionId?: string;
}

interface CoralAgent {
  name: string;
  status: string;
  description: string;
  links: string[];
}

interface CoralSessionExtended extends CoralSession {
  agents: CoralAgent[];
}

type AgentTuple = [string, AgentState];
type AgentWithMeta = [string, AgentState, AgentMeta];
type Tab = "local" | "coralos" | "messaging" | "shared-state" | "workflows" | "solana-pay" | "pay-demo" | "payment-flows" | "python-agent" | "weather";

interface PythonAgentEvent {
  type: string;
  [key: string]: unknown;
}

interface PaymentFlowRecord {
  id: string;
  agent_id: string;
  endpoint: string;
  status: string;
  protocol: string | null;
  amount: number | null;
  recipient: string | null;
  token: string | null;
  payment_header: string | null;
  response_body: string | null;
  error: string | null;
  request_at: string;
  challenge_at: string | null;
  payment_at: string | null;
  delivery_at: string | null;
}

const ROLES = ["leader", "coordinator", "worker", "monitor", "analyst", "trader"] as const;

function App() {
  const [tab, setTab] = useState<Tab>("weather");

  // --- Local Agents state ---
  const [agents, setAgents] = useState<AgentTuple[]>([]);
  const [agentsWithRoles, setAgentsWithRoles] = useState<AgentWithMeta[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [newId, setNewId] = useState("");
  const [loading, setLoading] = useState<Record<string, boolean>>({});

  // --- CoralOS state ---
  const [coralUrl, setCoralUrl] = useState("http://localhost:5555");
  const [coralToken, setCoralToken] = useState("");
  const [coralNamespace, setCoralNamespace] = useState("default");
  const [coralSessions, setCoralSessions] = useState<CoralSessionExtended[]>([]);
  const [selectedCoralSession, setSelectedCoralSession] = useState<string | null>(null);
  const [coralLoading, setCoralLoading] = useState(false);

  // --- MCP join state ---
  const [mcpConnectionUrl, setMcpConnectionUrl] = useState('');
  const [mcpAgentName, setMcpAgentName] = useState('');
  const [mcpJoining, setMcpJoining] = useState(false);
  const [mcpStatuses, setMcpStatuses] = useState<Record<string, boolean>>({});

  // --- Python side-car agent state ---
  const [pyAgent, setPyAgent] = useState("helius-monitor");
  const [pyWallet, setPyWallet] = useState("");
  const [pyAmount, setPyAmount] = useState("0.5");
  const [pyHeliusKey, setPyHeliusKey] = useState("");
  const [pyRpcUrl, setPyRpcUrl] = useState("");

  // --- Weather agent state ---
  const [weatherCity, setWeatherCity] = useState("London");
  const [weatherResult, setWeatherResult] = useState<Record<string, unknown> | null>(null);
  const [weatherLoading, setWeatherLoading] = useState(false);
  const [weatherError, setWeatherError] = useState("");
  const [pyWsUrl, setPyWsUrl] = useState("");
  const [pyMode, setPyMode] = useState("standalone");
  const [pyRunning, setPyRunning] = useState(false);
  const [pyEvents, setPyEvents] = useState<PythonAgentEvent[]>([]);
  const [pyError, setPyError] = useState<string | null>(null);

  // --- Messaging state ---
  const [messages, setMessages] = useState<AgentMessage[]>([]);
  const [msgFrom, setMsgFrom] = useState("");
  const [msgTo, setMsgTo] = useState("");
  const [msgType, setMsgType] = useState("chat");
  const [msgPayload, setMsgPayload] = useState("");
  const [msgBroadcast, setMsgBroadcast] = useState(false);
  const [msgFilterAgent, setMsgFilterAgent] = useState<string | null>(null);

  // --- Shared State ---
  const [sharedState, setSharedState] = useState<Record<string, SharedStateEntry>>({});
  const [stateHistory, setStateHistory] = useState<StateChange[]>([]);
  const [newStateKey, setNewStateKey] = useState("");
  const [newStateValue, setNewStateValue] = useState("");
  const [stateChangedBy, setStateChangedBy] = useState("");

  // --- Workflows ---
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [selectedWorkflow, setSelectedWorkflow] = useState<string | null>(null);
  const [newWorkflowName, setNewWorkflowName] = useState("");
  const [newWorkflowDesc, setNewWorkflowDesc] = useState("");
  const [newWorkflowSteps, setNewWorkflowSteps] = useState<{ name: string; desc: string }[]>([
    { name: "", desc: "" },
  ]);

  // --- Solana Pay demo state ---
  const [spAgentId, setSpAgentId] = useState("pay-agent");
  const [spMode, setSpMode] = useState<"Transfer" | "Payment">("Transfer");
  const [spRecipient, setSpRecipient] = useState("");
  const [spAmount, setSpAmount] = useState("0.01");
  const [spLabel, setSpLabel] = useState("");
  const [spMessage, setSpMessage] = useState("");
  const [spGeneratedUrl, setSpGeneratedUrl] = useState("");
  const [spParseUrl, setSpParseUrl] = useState("");
  const [spParsedResult, setSpParsedResult] = useState<any>(null);
  const [spSignature, setSpSignature] = useState("");
  const [spRpcUrl, setSpRpcUrl] = useState("https://api.devnet.solana.com");
  const [spExpectedRecipient, setSpExpectedRecipient] = useState("");
  const [spValidationResult, setSpValidationResult] = useState<any>(null);
  const [sp402Headers, setSp402Headers] = useState("");
  const [sp402Result, setSp402Result] = useState<any>(null);
  const [spDemoEndpoint, setSpDemoEndpoint] = useState("https://debugger.pay.sh/mpp/quote/AAPL");
  const [spDemoBudget, setSpDemoBudget] = useState("1000000");
  const [spDemoResult, setSpDemoResult] = useState<any>(null);

  // --- Pay Demo state ---
  const DEMO_API_KEY = import.meta.env.VITE_HELIUS_API_KEY ?? "5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5";
  const [pdSellerId] = useState("pd-seller");
  const [pdBuyerId] = useState("pd-buyer");
  const [pdRecipient, setPdRecipient] = useState("");
  const [pdAmount, setPdAmount] = useState("0.001");
  const [pdApiKey, setPdApiKey] = useState(DEMO_API_KEY);
  const [pdLabel, setPdLabel] = useState("DataFeed");
  const [pdSellerState, setPdSellerState] = useState<AgentState | null>(null);
  const [pdBuyerState, setPdBuyerState] = useState<AgentState | null>(null);
  const [pdSaleCompleted, setPdSaleCompleted] = useState(false);
  const [pdSolanaPayUrl, setPdSolanaPayUrl] = useState("");
  const [pdSellerCreated, setPdSellerCreated] = useState(false);
  const [pdBuyerCreated, setPdBuyerCreated] = useState(false);
  const [pdDataResult, setPdDataResult] = useState<string | null>(null);

  // --- Payment Flows debugger state ---
  const [paymentFlows, setPaymentFlows] = useState<PaymentFlowRecord[]>([]);
  const [selectedFlowId, setSelectedFlowId] = useState<string | null>(null);

  const refreshLocal = async () => {
    const list = await invoke<AgentTuple[]>("list_agents");
    setAgents(list);
    const withRoles = await invoke<AgentWithMeta[]>("list_agents_with_roles");
    setAgentsWithRoles(withRoles);
    if (selectedId && !list.find(([id]) => id === selectedId)) {
      setSelectedId(null);
    }
  };

  const refreshCoral = async () => {
    if (!coralNamespace) return;
    setCoralLoading(true);
    try {
      const sessions = await invoke<CoralSessionExtended[]>("coralos_list_sessions", {
        namespace: coralNamespace,
      });
      setCoralSessions(sessions);
    } catch (e) {
      console.error("CoralOS fetch failed:", e);
      setCoralSessions([]);
    } finally {
      setCoralLoading(false);
    }
  };

  const refreshMessages = async () => {
    const msgs = await invoke<AgentMessage[]>("get_all_messages");
    setMessages(msgs);
  };

  const refreshSharedState = async () => {
    const state = await invoke<Record<string, SharedStateEntry>>("get_all_shared_state");
    setSharedState(state);
    const history = await invoke<StateChange[]>("get_state_history");
    setStateHistory(history);
  };

  const refreshWorkflows = async () => {
    const wfs = await invoke<Workflow[]>("list_workflows");
    setWorkflows(wfs);
  };

  useEffect(() => {
    refreshLocal();
    const id = setInterval(refreshLocal, 2000);
    return () => clearInterval(id);
  }, []);

  // Listen for streamed events from the Python side-car agent (Tauri only).
  useEffect(() => {
    const unlisten = listenEvent<PythonAgentEvent>("python-agent-event", (e) => {
      const ev = e.payload;
      setPyEvents((prev) => [...prev.slice(-199), ev]);
      if (ev.type === "exited") setPyRunning(false);
    });
    if (IS_TAURI) {
      invoke<boolean>("python_agent_status").then(setPyRunning).catch(() => {});
    }
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const startPyAgent = async () => {
    setPyError(null);
    setPyEvents([]);
    try {
      await invoke("python_agent_start", {
        config: {
          agent: pyAgent,
          wallet: pyWallet.trim(),
          amount_sol: parseFloat(pyAmount) || 0,
          rpc_url: pyRpcUrl.trim(),
          ws_url: pyWsUrl.trim(),
          helius_api_key: pyHeliusKey.trim(),
          mode: pyMode,
          coral_url: "",
        },
      });
      setPyRunning(true);
    } catch (e) {
      setPyError(String(e));
    }
  };

  const stopPyAgent = async () => {
    try {
      await invoke("python_agent_stop");
    } finally {
      setPyRunning(false);
    }
  };

  useEffect(() => {
    if (tab !== "coralos") return;
    refreshCoral();
    const id = setInterval(refreshCoral, 5000);
    return () => clearInterval(id);
  }, [tab, coralNamespace, coralUrl, coralToken]);

  useEffect(() => {
    if (tab !== "messaging") return;
    refreshMessages();
    const id = setInterval(refreshMessages, 2000);
    return () => clearInterval(id);
  }, [tab]);

  useEffect(() => {
    if (tab !== "shared-state") return;
    refreshSharedState();
    const id = setInterval(refreshSharedState, 3000);
    return () => clearInterval(id);
  }, [tab]);

  useEffect(() => {
    if (tab !== "workflows") return;
    refreshWorkflows();
    const id = setInterval(refreshWorkflows, 3000);
    return () => clearInterval(id);
  }, [tab]);

  useEffect(() => {
    if (tab !== "pay-demo") return;
    const poll = async () => {
      try {
        const seller = await invoke<AgentState>("get_agent_state", { id: pdSellerId });
        setPdSellerState(seller);
        const url = seller.actions.find((a: AgentAction) => a.action_type === "url-generated")?.details ?? "";
        if (url) setPdSolanaPayUrl(url);
        if (!pdSaleCompleted && seller.actions.some((a: AgentAction) => a.action_type === "payment-received")) {
          const txSig = seller.actions.find((a: AgentAction) => a.action_type === "payment-received")?.tx_signature ?? null;
          try {
            const data = await invoke<string>("complete_sale", {
              sellerId: pdSellerId,
              buyerId: pdBuyerId,
              txSignature: txSig,
            });
            setPdDataResult(data);
          } catch (e) {
            setPdDataResult(JSON.stringify({ error: String(e) }));
          }
          setPdSaleCompleted(true);
        }
      } catch (_) {}
      try {
        const buyer = await invoke<AgentState>("get_agent_state", { id: pdBuyerId });
        setPdBuyerState(buyer);
      } catch (_) {}
    };
    poll();
    const id = setInterval(poll, 2000);
    return () => clearInterval(id);
  }, [tab, pdSaleCompleted]);

  useEffect(() => {
    if (tab !== "payment-flows") return;
    const poll = async () => {
      try {
        const flows = await invoke<PaymentFlowRecord[]>("get_payment_flows");
        setPaymentFlows(flows);
      } catch (_) {}
    };
    poll();
    const id = setInterval(poll, 2000);
    return () => clearInterval(id);
  }, [tab]);

  const selectedAgent = agents.find(([id]) => id === selectedId);
  const selectedCoral = coralSessions.find((s) => s.id === selectedCoralSession);

  const handleCreate = async () => {
    if (!newId.trim()) return;
    await invoke("create_agent", { id: newId.trim() });
    setNewId("");
    await refreshLocal();
  };

  const handleDelete = async (id: string) => {
    await invoke("delete_agent", { id });
    if (selectedId === id) setSelectedId(null);
    await refreshLocal();
  };

  const handleStart = async (id: string) => {
    setLoading((p) => ({ ...p, [id]: true }));
    await invoke("start_agent", { id });
    await refreshLocal();
    setLoading((p) => ({ ...p, [id]: false }));
  };

  const handleStop = async (id: string) => {
    setLoading((p) => ({ ...p, [id]: true }));
    await invoke("stop_agent", { id });
    await refreshLocal();
    setLoading((p) => ({ ...p, [id]: false }));
  };

  const handleHelius = async (id: string) => {
    await invoke("set_agent_helius", { id, apiKey: import.meta.env.VITE_HELIUS_API_KEY ?? "" });
    await refreshLocal();
  };

  const handleSetRole = async (id: string, role: string) => {
    await invoke("set_agent_role", { id, role });
    await refreshLocal();
  };

  const handleSendMessage = async () => {
    if (!msgFrom.trim() || !msgPayload.trim()) return;
    await invoke("send_message", {
      from: msgFrom,
      to: msgBroadcast ? null : msgTo || null,
      msgType: msgType,
      payload: msgPayload,
    });
    setMsgPayload("");
    await refreshMessages();
  };

  const handleSetSharedState = async () => {
    if (!newStateKey.trim() || !newStateValue.trim() || !stateChangedBy.trim()) return;
    try {
      const value = JSON.parse(newStateValue);
      await invoke("set_shared_state", {
        key: newStateKey,
        value,
        changedBy: stateChangedBy,
      });
      setNewStateKey("");
      setNewStateValue("");
      await refreshSharedState();
    } catch {
      await invoke("set_shared_state", {
        key: newStateKey,
        value: newStateValue,
        changedBy: stateChangedBy,
      });
      setNewStateKey("");
      setNewStateValue("");
      await refreshSharedState();
    }
  };

  const handleCreateWorkflow = async () => {
    if (!newWorkflowName.trim() || newWorkflowSteps.length === 0) return;
    const id = "wf-" + Math.random().toString(36).slice(2, 8);
    const steps: WorkflowStep[] = newWorkflowSteps
      .filter((s) => s.name.trim())
      .map((s, i) => ({
        id: `step-${i}`,
        name: s.name,
        description: s.desc,
        status: "Pending",
        assigned_to: null,
        dependencies: [],
        result: null,
        started_at: null,
        completed_at: null,
        timeout_secs: null,
      }));
    await invoke("create_workflow", {
      id,
      name: newWorkflowName,
      description: newWorkflowDesc,
      steps,
      priority: 5,
      createdBy: "user",
    });
    setNewWorkflowName("");
    setNewWorkflowDesc("");
    setNewWorkflowSteps([{ name: "", desc: "" }]);
    await refreshWorkflows();
  };

  const handleAssignStep = async (wfId: string, stepId: string, agentId: string) => {
    await invoke("assign_workflow_step", { workflowId: wfId, stepId, agentId });
    await refreshWorkflows();
  };

  const handleStartStep = async (wfId: string, stepId: string) => {
    await invoke("start_workflow_step", { workflowId: wfId, stepId });
    await refreshWorkflows();
  };

  const handleCompleteStep = async (wfId: string, stepId: string) => {
    await invoke("complete_workflow_step", {
      workflowId: wfId,
      stepId,
      result: "completed",
    });
    await refreshWorkflows();
  };

  const handleCoralConnect = async () => {
    await invoke("coralos_set_url", { url: coralUrl });
    await invoke("coralos_set_token", { token: coralToken });
    await refreshCoral();
  };

  const handleMcpJoin = async () => {
    if (!mcpConnectionUrl.trim() || !mcpAgentName.trim()) return;
    setMcpJoining(true);
    try {
      await invoke('coralos_mcp_join', { connectionUrl: mcpConnectionUrl.trim(), agentName: mcpAgentName.trim() });
      const active = await invoke<boolean>('coralos_mcp_status', { name: mcpAgentName.trim() });
      setMcpStatuses(p => ({ ...p, [mcpAgentName.trim()]: active }));
    } finally {
      setMcpJoining(false);
    }
  };

  // --- Weather handler ---
  const handleWeatherQuery = async () => {
    if (!weatherCity.trim()) return;
    setWeatherLoading(true);
    setWeatherError("");
    setWeatherResult(null);
    try {
      const data = await invoke<Record<string, unknown>>("weather_query", { city: weatherCity.trim() });
      setWeatherResult(data);
    } catch (e) {
      setWeatherError(String(e));
    } finally {
      setWeatherLoading(false);
    }
  };

  // --- Solana Pay handlers ---
  const handleCreateSolanaPayAgent = async () => {
    try {
      await invoke("create_solana_pay_agent", { id: spAgentId, mode: spMode });
      await refreshLocal();
    } catch (e) {
      console.error("Failed to create Solana Pay agent:", e);
    }
  };

  const handleCreateUrl = async () => {
    try {
      const url = await invoke("solana_pay_create_url", {
        recipient: spRecipient,
        amount: parseFloat(spAmount),
        label: spLabel || undefined,
        message: spMessage || undefined,
      });
      setSpGeneratedUrl(url as string);
    } catch (e) {
      console.error("Failed to create URL:", e);
    }
  };

  const handleParseUrl = async () => {
    try {
      const parsed = await invoke("solana_pay_parse_url", { url: spParseUrl });
      setSpParsedResult(parsed);
    } catch (e) {
      console.error("Failed to parse URL:", e);
    }
  };

  const handleValidateTx = async () => {
    try {
      const result = await invoke("solana_pay_validate", {
        id: spAgentId,
        signature: spSignature,
        expectedRecipient: spExpectedRecipient || undefined,
      });
      setSpValidationResult(result);
    } catch (e) {
      console.error("Failed to validate transaction:", e);
    }
  };

  const handleParse402 = async () => {
    try {
      const headers = JSON.parse(sp402Headers);
      const result = await invoke("x402_parse_challenge", { headers });
      setSp402Result(result);
    } catch (e) {
      console.error("Failed to parse 402:", e);
    }
  };

  const handleDemoPayment = async () => {
    try {
      const result = await invoke("x402_demo_payment", {
        endpoint: spDemoEndpoint,
        budget: parseInt(spDemoBudget),
      });
      setSpDemoResult(result);
    } catch (e) {
      console.error("Failed to run demo payment:", e);
    }
  };

  const filteredMessages = msgFilterAgent
    ? messages.filter(
        (m) =>
          m.from === msgFilterAgent ||
          m.to === msgFilterAgent ||
          (m.to === null && m.from !== msgFilterAgent)
      )
    : messages;

  const roleBadge = (role: string) => {
    const colors: Record<string, string> = {
      leader: "bg-purple-900/50 text-purple-300",
      coordinator: "bg-indigo-900/50 text-indigo-300",
      worker: "bg-blue-900/50 text-blue-300",
      monitor: "bg-yellow-900/50 text-yellow-300",
      analyst: "bg-green-900/50 text-green-300",
      trader: "bg-red-900/50 text-red-300",
    };
    return colors[role] || "bg-gray-700 text-gray-300";
  };

  const actionBadgeClass = (type: string) => {
    const key = type.replace(/-/g, "-");
    const known: Record<string, string> = {
      "payment-received":      "type-payment-received",
      "data-delivered":        "type-data-delivered",
      "data-received":         "type-data-received",
      "data-request":          "type-data-request",
      "url-generated":         "type-url-generated",
      "coral-mention":         "type-url-generated",
      "coral-url-generated":   "type-data-delivered",
      "coral-payment-result":  "type-payment-received",
      "strategy-start":        "type-strategy-start",
      "poll-tick":             "type-poll-tick",
      "poll-error":            "type-poll-error",
      "rpc-error":             "type-rpc-error",
    };
    return `action-badge ${known[key] ?? "type-default"}`;
  };

  const [copiedUrl, setCopiedUrl] = useState(false);
  const copyUrl = (url: string) => {
    navigator.clipboard.writeText(url);
    setCopiedUrl(true);
    setTimeout(() => setCopiedUrl(false), 1500);
  };

  return (
    <div className="app-shell">
      <header className="app-header">
        <h1 className="app-title">Agent Economy</h1>
        <p className="app-subtitle">
          Multi-agent Solana runtime — payments, messaging, shared state, and workflow orchestration
        </p>
      </header>

      {/* Tab bar */}
      <div className="tab-bar">
        {[
          { key: "weather",        label: "🌤 Weather"     },
          { key: "pay-demo",       label: "⚡ Pay Demo"    },
          { key: "local",          label: "Agents"         },
          { key: "coralos",        label: "CoralOS"        },
          { key: "workflows",      label: "Workflows"      },
          { key: "solana-pay",     label: "Solana Pay"     },
          { key: "messaging",      label: "Messaging"      },
          { key: "shared-state",   label: "Shared State"   },
          { key: "payment-flows",  label: "Flows"          },
          { key: "python-agent",   label: "🐍 Python"      },
        ].map((t) => (
          <button
            key={t.key}
            className={`tab-btn${tab === t.key ? " active" : ""}`}
            onClick={() => setTab(t.key as Tab)}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === "local" && (
        <>
          <div className="card mb-5 flex items-center gap-3">
            <input
              className="input-field flex-1 max-w-sm"
              placeholder="Agent ID (e.g. nft-floor-monitor)"
              value={newId}
              onChange={(e) => setNewId(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
            />
            <button className="btn-primary" onClick={handleCreate}>
              Create Agent
            </button>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-3 gap-5">
            <div className="lg:col-span-1 space-y-3">
              <h2 className="section-title">Agents <span style={{color:"var(--text-dim)",fontWeight:400}}>({agents.length})</span></h2>
              {agentsWithRoles.length === 0 && (
                <p className="feed-empty">No agents yet. Create one above.</p>
              )}
              {agentsWithRoles.map(([id, state, meta]) => (
                <div
                  key={id}
                  className={`card cursor-pointer${selectedId === id ? " card-selected" : ""}`}
                  onClick={() => setSelectedId(id)}
                >
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <span className={`status-dot ${state.is_running ? "running" : "stopped"}`} />
                      <span className="mono font-semibold truncate max-w-[130px]" style={{fontSize:"12px",color:"var(--text-primary)"}}>
                        {id}
                      </span>
                      <span className={`role-badge ${roleBadge(meta.role)}`}>
                        {meta.role}
                      </span>
                    </div>
                    <button
                      className="mono"
                      style={{fontSize:"11px",color:"var(--red)",opacity:0.7,cursor:"pointer"}}
                      onClick={(e) => { e.stopPropagation(); handleDelete(id); }}
                    >
                      ✕
                    </button>
                  </div>
                  <div className="mono space-y-1" style={{fontSize:"11px",color:"var(--text-dim)"}}>
                    <div>Network: <span style={{color:"var(--blue)"}}>{state.network}</span></div>
                    <div>Actions: <span style={{color:"var(--text-primary)"}}>{state.actions.length}</span></div>
                    <div className="truncate">{state.rpc_endpoint}</div>
                  </div>
                  <div className="flex gap-2 flex-wrap mt-3">
                    <button className="btn-primary" style={{fontSize:"11px",padding:"4px 10px"}}
                      onClick={(e) => { e.stopPropagation(); handleStart(id); }}
                      disabled={state.is_running || loading[id]}>
                      Start
                    </button>
                    <button className="btn-danger" style={{fontSize:"11px",padding:"4px 10px"}}
                      onClick={(e) => { e.stopPropagation(); handleStop(id); }}
                      disabled={!state.is_running || loading[id]}>
                      Stop
                    </button>
                    <button className="btn-secondary" style={{fontSize:"11px",padding:"4px 10px"}}
                      onClick={(e) => { e.stopPropagation(); handleHelius(id); }}>
                      Helius
                    </button>
                  </div>
                  <div className="mt-2">
                    <select
                      className="input-field monospace"
                      style={{width:"auto",fontSize:"11px",padding:"3px 28px 3px 8px"}}
                      value={meta.role}
                      onChange={(e) => { e.stopPropagation(); handleSetRole(id, e.target.value); }}
                      onClick={(e) => e.stopPropagation()}
                    >
                      {ROLES.map((r) => (
                        <option key={r} value={r}>{r}</option>
                      ))}
                    </select>
                  </div>
                </div>
              ))}
            </div>

            <div className="lg:col-span-2">
              {selectedAgent ? (
                <AgentDetail id={selectedAgent[0]} state={selectedAgent[1]} />
              ) : (
                <div className="card flex items-center justify-center" style={{minHeight:"240px",color:"var(--text-dim)",fontSize:"13px"}}>
                  Select an agent to view details and action logs.
                </div>
              )}
            </div>
          </div>
        </>
      )}

      {tab === "coralos" && (
        <div className="space-y-5">
          <div className="card flex flex-wrap items-center gap-3">
            <input className="input-field flex-1" style={{minWidth:"200px"}} placeholder="CoralOS URL" value={coralUrl} onChange={(e) => setCoralUrl(e.target.value)} />
            <input className="input-field flex-1" style={{minWidth:"200px"}} placeholder="API Token" type="password" value={coralToken} onChange={(e) => setCoralToken(e.target.value)} />
            <input className="input-field" style={{width:"130px"}} placeholder="Namespace" value={coralNamespace} onChange={(e) => setCoralNamespace(e.target.value)} />
            <button className="btn-primary" onClick={handleCoralConnect}>Connect</button>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-3 gap-5">
            <div className="lg:col-span-1 space-y-3">
              <h2 className="section-title">Sessions <span style={{color:"var(--text-dim)",fontWeight:400}}>({coralSessions.length})</span></h2>
              {coralLoading && <p className="feed-empty">Loading…</p>}
              {!coralLoading && coralSessions.length === 0 && <p className="feed-empty">No sessions found.</p>}
              {coralSessions.map((session) => (
                <div
                  key={session.id}
                  className={`card cursor-pointer${selectedCoralSession === session.id ? " card-selected" : ""}`}
                  onClick={() => setSelectedCoralSession(session.id)}
                >
                  <div className="flex items-center gap-2 mb-2">
                    <span className={`status-dot ${session.status === "running" ? "running" : "stopped"}`} />
                    <span className="mono font-semibold truncate" style={{fontSize:"12px"}}>{session.id.slice(0, 8)}…</span>
                  </div>
                  <div className="mono space-y-1" style={{fontSize:"11px",color:"var(--text-dim)"}}>
                    <div>Namespace: <span style={{color:"var(--blue)"}}>{session.namespace}</span></div>
                    <div>Agents: <span style={{color:"var(--text-primary)"}}>{session.agentCount ?? "—"}</span></div>
                  </div>
                </div>
              ))}
            </div>
            <div className="lg:col-span-2">
              {selectedCoral ? (
                <CoralSessionDetail session={selectedCoral} />
              ) : (
                <div className="card flex items-center justify-center" style={{minHeight:"200px",color:"var(--text-dim)",fontSize:"13px"}}>
                  Select a session.
                </div>
              )}
            </div>
          </div>

          <div className="card space-y-3">
            <h2 className="section-title">Join Swarm as Rust Agent</h2>
            <p className="text-xs text-gray-500">
              Paste the CORAL_CONNECTION_URL from a running CoralOS session. Select a local agent — it will receive mentions and reply using its strategy.
            </p>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div>
                <label className="block text-xs text-gray-500 mb-1">CORAL_CONNECTION_URL</label>
                <input
                  className="input-field"
                  placeholder="http://localhost:5555/mcp?..."
                  value={mcpConnectionUrl}
                  onChange={e => setMcpConnectionUrl(e.target.value)}
                />
              </div>
              <div>
                <label className="block text-xs text-gray-500 mb-1">Local agent to use</label>
                <select
                  className="input-field"
                  value={mcpAgentName}
                  onChange={e => setMcpAgentName(e.target.value)}
                >
                  <option value="">Select agent…</option>
                  {agentsWithRoles.map(([id, , meta]) => (
                    <option key={id} value={id}>{id} ({meta.role})</option>
                  ))}
                </select>
              </div>
            </div>
            <div className="flex items-center gap-3">
              <button className="btn-primary" onClick={handleMcpJoin} disabled={mcpJoining}>
                {mcpJoining ? 'Joining…' : 'Join Swarm'}
              </button>
              {Object.entries(mcpStatuses).map(([name, active]) => (
                <span key={name} className="flex items-center gap-1 text-xs font-mono">
                  <span className={`status-dot ${active ? 'running' : 'stopped'}`} />
                  {name}
                </span>
              ))}
            </div>
          </div>
        </div>
      )}

      {tab === "messaging" && (
        <div className="space-y-5">
          <div className="card">
            <h2 className="section-title" style={{marginBottom:"14px"}}>Compose Message</h2>
            <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
              <input className="input-field" placeholder="From agent" value={msgFrom} onChange={(e) => setMsgFrom(e.target.value)} />
              <input className="input-field" placeholder="To agent (or broadcast)" value={msgBroadcast ? "" : msgTo} disabled={msgBroadcast} onChange={(e) => setMsgTo(e.target.value)} />
              <input className="input-field" placeholder="Type (e.g. chat, task, alert)" value={msgType} onChange={(e) => setMsgType(e.target.value)} />
              <div className="flex items-center gap-2">
                <label className="flex items-center gap-2 cursor-pointer" style={{fontSize:"13px",color:"var(--text-secondary)"}}>
                  <input type="checkbox" checked={msgBroadcast} onChange={(e) => setMsgBroadcast(e.target.checked)} />
                  Broadcast
                </label>
              </div>
            </div>
            <textarea
              className="input-field mt-3"
              placeholder="Message payload…"
              value={msgPayload}
              onChange={(e) => setMsgPayload(e.target.value)}
            />
            <button className="btn-primary mt-3" onClick={handleSendMessage}>Send</button>
          </div>

          <div className="card">
            <div className="flex items-center justify-between mb-4">
              <h2 className="section-title">Message Log <span style={{color:"var(--text-dim)",fontWeight:400}}>({filteredMessages.length})</span></h2>
              <select
                className="input-field monospace"
                style={{width:"auto",fontSize:"11px"}}
                value={msgFilterAgent || ""}
                onChange={(e) => setMsgFilterAgent(e.target.value || null)}
              >
                <option value="">All messages</option>
                {agentsWithRoles.map(([id]) => (
                  <option key={id} value={id}>{id}</option>
                ))}
              </select>
            </div>
            <div className="overflow-x-auto max-h-[500px] overflow-y-auto">
              <table className="data-table">
                <thead className="sticky top-0" style={{background:"var(--bg-elevated)"}}>
                  <tr className="">
                    <th className="py-2 px-3">Time</th>
                    <th className="py-2 px-3">From</th>
                    <th className="py-2 px-3">To</th>
                    <th className="py-2 px-3">Type</th>
                    <th className="py-2 px-3">Payload</th>
                  </tr>
                </thead>
                <tbody>
                  {[...filteredMessages].reverse().map((m, i) => (
                    <tr key={i} className="">
                      <td className="py-2 px-3 font-mono text-gray-400 text-xs">
                        {new Date(m.timestamp).toLocaleTimeString()}
                      </td>
                      <td className="py-2 px-3 font-semibold text-blue-300">{m.from}</td>
                      <td className="py-2 px-3 text-gray-300">
                        {m.to ?? (
                          <span className="inline-block px-1.5 py-0.5 rounded text-[10px] bg-purple-900/50 text-purple-300">
                            BROADCAST
                          </span>
                        )}
                      </td>
                      <td className="py-2 px-3">
                        <span className="inline-block px-2 py-0.5 rounded text-xs bg-gray-700 text-gray-300">
                          {m.msg_type}
                        </span>
                      </td>
                      <td className="py-2 px-3 max-w-md truncate" title={m.payload}>
                        {m.payload}
                      </td>
                    </tr>
                  ))}
                  {filteredMessages.length === 0 && (
                    <tr>
                      <td colSpan={5} className="feed-empty">
                        No messages yet.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {tab === "shared-state" && (
        <div className="space-y-5">
          <div className="card">
            <h2 className="section-title" style={{marginBottom:"14px"}}>Set Shared State</h2>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              <input
                className="input-field"
                placeholder="Key"
                value={newStateKey}
                onChange={(e) => setNewStateKey(e.target.value)}
              />
              <input
                className="input-field"
                placeholder="Value (JSON or string)"
                value={newStateValue}
                onChange={(e) => setNewStateValue(e.target.value)}
              />
              <input
                className="input-field"
                placeholder="Changed by (agent id)"
                value={stateChangedBy}
                onChange={(e) => setStateChangedBy(e.target.value)}
              />
            </div>
            <button className="btn-primary mt-3" onClick={handleSetSharedState}>
              Set State
            </button>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <div className="card">
              <h2 className="section-title" style={{marginBottom:"14px"}}>
                Shared State ({Object.keys(sharedState).length} keys)
              </h2>
              <div className="overflow-x-auto max-h-[400px] overflow-y-auto">
                <table className="data-table">
                  <thead className="sticky top-0" style={{background:"var(--bg-elevated)"}}>
                    <tr className="">
                      <th className="py-2 px-3">Key</th>
                      <th className="py-2 px-3">Value</th>
                      <th className="py-2 px-3">Modified By</th>
                      <th className="py-2 px-3">Version</th>
                    </tr>
                  </thead>
                  <tbody>
                    {Object.entries(sharedState).map(([key, entry], i) => (
                      <tr key={i} className="">
                        <td className="mono" style={{color:"var(--blue)"}}>{key}</td>
                        <td className="py-2 px-3 max-w-xs truncate" title={JSON.stringify(entry.value)}>
                          {JSON.stringify(entry.value)}
                        </td>
                        <td className="" style={{color:"var(--text-secondary)"}}>{entry.modified_by}</td>
                        <td className="mono" style={{color:"var(--text-dim)"}}>{entry.version}</td>
                      </tr>
                    ))}
                    {Object.keys(sharedState).length === 0 && (
                      <tr>
                        <td colSpan={4} className="feed-empty">
                          No shared state entries.
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            </div>

            <div className="card">
              <h2 className="section-title" style={{marginBottom:"14px"}}>Change History</h2>
              <div className="overflow-x-auto max-h-[400px] overflow-y-auto">
                <table className="data-table">
                  <thead className="sticky top-0" style={{background:"var(--bg-elevated)"}}>
                    <tr className="">
                      <th className="py-2 px-3">Time</th>
                      <th className="py-2 px-3">Key</th>
                      <th className="py-2 px-3">By</th>
                    </tr>
                  </thead>
                  <tbody>
                    {[...stateHistory].reverse().map((change, i) => (
                      <tr key={i} className="">
                        <td className="py-2 px-3 font-mono text-gray-400 text-xs">
                          {new Date(change.timestamp).toLocaleTimeString()}
                        </td>
                        <td className="mono" style={{color:"var(--blue)"}}>{change.key}</td>
                        <td className="" style={{color:"var(--text-secondary)"}}>{change.changed_by}</td>
                      </tr>
                    ))}
                    {stateHistory.length === 0 && (
                      <tr>
                        <td colSpan={3} className="feed-empty">
                          No changes recorded.
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </div>
      )}

      {tab === "workflows" && (
        <div className="space-y-5">
          <div className="card">
            <h2 className="section-title" style={{marginBottom:"14px"}}>Create Workflow</h2>
            <input
              className="input-field mb-2"
              placeholder="Workflow name"
              value={newWorkflowName}
              onChange={(e) => setNewWorkflowName(e.target.value)}
            />
            <input
              className="input-field mb-2"
              placeholder="Description"
              value={newWorkflowDesc}
              onChange={(e) => setNewWorkflowDesc(e.target.value)}
            />
            <div className="space-y-2">
              {newWorkflowSteps.map((step, i) => (
                <div key={i} className="flex gap-2">
                  <input
                    className="input-field flex-1"
                    placeholder={`Step ${i + 1} name`}
                    value={step.name}
                    onChange={(e) => {
                      const s = [...newWorkflowSteps];
                      s[i].name = e.target.value;
                      setNewWorkflowSteps(s);
                    }}
                  />
                  <input
                    className="input-field flex-1"
                    placeholder="Description"
                    value={step.desc}
                    onChange={(e) => {
                      const s = [...newWorkflowSteps];
                      s[i].desc = e.target.value;
                      setNewWorkflowSteps(s);
                    }}
                  />
                  {newWorkflowSteps.length > 1 && (
                    <button
                      className="btn-danger text-xs py-1 px-2"
                      onClick={() => {
                        const s = [...newWorkflowSteps];
                        s.splice(i, 1);
                        setNewWorkflowSteps(s);
                      }}
                    >
                      Remove
                    </button>
                  )}
                </div>
              ))}
            </div>
            <div className="flex gap-2 mt-3">
              <button
                className="btn-secondary text-xs"
                onClick={() =>
                  setNewWorkflowSteps([...newWorkflowSteps, { name: "", desc: "" }])
                }
              >
                + Add Step
              </button>
              <button className="btn-primary text-xs" onClick={handleCreateWorkflow}>
                Create Workflow
              </button>
            </div>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
            <div className="lg:col-span-1 space-y-3">
              <h2 className="section-title">Workflows ({workflows.length})</h2>
              {workflows.length === 0 && (
                <p className="feed-empty">No workflows yet.</p>
              )}
              {workflows.map((wf) => (
                <div
                  key={wf.id}
                  className={`card cursor-pointer transition ${
                    selectedWorkflow === wf.id ? "border-blue-500" : ""
                  }`}
                  onClick={() => setSelectedWorkflow(wf.id)}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="font-mono text-sm font-semibold truncate">{wf.name}</span>
                    <span
                      className={`inline-block px-2 py-0.5 rounded text-xs font-semibold ${
                        wf.status === "completed"
                          ? "bg-green-900/50 text-green-300"
                          : wf.status === "failed"
                          ? "bg-red-900/50 text-red-300"
                          : wf.status === "running"
                          ? "bg-blue-900/50 text-blue-300"
                          : "bg-gray-700 text-gray-300"
                      }`}
                    >
                      {wf.status}
                    </span>
                  </div>
                  <div className="text-xs text-gray-400">
                    Priority: {wf.priority} | Steps: {wf.steps.length} | Progress: {Math.round(
                      (wf.steps.filter((s) => s.status === "Completed").length /
                        Math.max(wf.steps.length, 1)) *
                        100
                    )}%
                  </div>
                </div>
              ))}
            </div>
            <div className="lg:col-span-2">
              {selectedWorkflow ? (
                <WorkflowDetail
                  workflow={workflows.find((w) => w.id === selectedWorkflow)!}
                  agents={agentsWithRoles.map(([id]) => id)}
                  onAssign={handleAssignStep}
                  onStart={handleStartStep}
                  onComplete={handleCompleteStep}
                />
              ) : (
                <div className="card h-full flex items-center justify-center text-gray-500">
                  Select a workflow to view details and manage steps.
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {tab === "solana-pay" && (
        <div className="space-y-5">
          <div className="card">
            <h2 className="section-title" style={{marginBottom:"14px"}}>Create Solana Pay Agent</h2>
            <div className="flex gap-3 items-end flex-wrap">
              <div className="flex-1 min-w-[200px]">
                <label className="block text-xs text-gray-500 mb-1">Agent ID</label>
                <input
                  className="input-field"
                  value={spAgentId}
                  onChange={(e) => setSpAgentId(e.target.value)}
                />
              </div>
              <div className="min-w-[150px]">
                <label className="block text-xs text-gray-500 mb-1">Mode</label>
                <select
                  className="input-field"
                  value={spMode}
                  onChange={(e) => setSpMode(e.target.value as "Transfer" | "Payment")}
                >
                  <option value="Transfer">Transfer</option>
                  <option value="Payment">Payment</option>
                </select>
              </div>
              <button className="btn-primary" onClick={handleCreateSolanaPayAgent}>
                Create Agent
              </button>
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="card">
              <h2 className="section-title" style={{marginBottom:"14px"}}>Generate Transfer URL</h2>
              <div className="space-y-3">
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Recipient</label>
                  <input
                    className="input-field"
                    placeholder="Solana address"
                    value={spRecipient}
                    onChange={(e) => setSpRecipient(e.target.value)}
                  />
                </div>
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Amount (SOL)</label>
                  <input
                    className="input-field"
                    type="number"
                    step="0.000000001"
                    value={spAmount}
                    onChange={(e) => setSpAmount(e.target.value)}
                  />
                </div>
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Label (optional)</label>
                  <input
                    className="input-field"
                    value={spLabel}
                    onChange={(e) => setSpLabel(e.target.value)}
                  />
                </div>
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Message (optional)</label>
                  <input
                    className="input-field"
                    value={spMessage}
                    onChange={(e) => setSpMessage(e.target.value)}
                  />
                </div>
                <button className="btn-primary w-full" onClick={handleCreateUrl}>
                  Generate URL
                </button>
                {spGeneratedUrl && (
                  <div className="bg-gray-800 rounded p-2 text-xs font-mono break-all text-green-400">
                    {spGeneratedUrl}
                  </div>
                )}
              </div>
            </div>

            <div className="card">
              <h2 className="section-title" style={{marginBottom:"14px"}}>Parse URL</h2>
              <div className="space-y-3">
                <div>
                  <label className="block text-xs text-gray-500 mb-1">solana: URL</label>
                  <input
                    className="input-field"
                    value={spParseUrl}
                    onChange={(e) => setSpParseUrl(e.target.value)}
                  />
                </div>
                <button className="btn-primary w-full" onClick={handleParseUrl}>
                  Parse URL
                </button>
                {spParsedResult && (
                  <pre className="bg-gray-800 rounded p-3 text-xs overflow-auto">
                    {JSON.stringify(spParsedResult, null, 2)}
                  </pre>
                )}
              </div>
            </div>

            <div className="card">
              <h2 className="section-title" style={{marginBottom:"14px"}}>Validate Transaction</h2>
              <div className="space-y-3">
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Transaction Signature</label>
                  <input
                    className="input-field"
                    value={spSignature}
                    onChange={(e) => setSpSignature(e.target.value)}
                  />
                </div>
                <div>
                  <label className="block text-xs text-gray-500 mb-1">RPC URL</label>
                  <input
                    className="input-field"
                    value={spRpcUrl}
                    onChange={(e) => setSpRpcUrl(e.target.value)}
                  />
                </div>
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Expected Recipient (optional)</label>
                  <input
                    className="input-field"
                    value={spExpectedRecipient}
                    onChange={(e) => setSpExpectedRecipient(e.target.value)}
                  />
                </div>
                <button className="btn-primary w-full" onClick={handleValidateTx}>
                  Validate
                </button>
                {spValidationResult && (
                  <pre className="bg-gray-800 rounded p-3 text-xs overflow-auto">
                    {JSON.stringify(spValidationResult, null, 2)}
                  </pre>
                )}
              </div>
            </div>

            <div className="card">
              <h2 className="section-title" style={{marginBottom:"14px"}}>Parse 402 Challenge</h2>
              <div className="space-y-3">
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Headers (JSON array)</label>
                  <textarea
                    className="input-field" style={{minHeight:"96px"}}
                    placeholder='[["www-authenticate", "Solana mpp=..."], ...]'
                    value={sp402Headers}
                    onChange={(e) => setSp402Headers(e.target.value)}
                  />
                </div>
                <button className="btn-primary w-full" onClick={handleParse402}>
                  Parse Challenge
                </button>
                {sp402Result && (
                  <pre className="bg-gray-800 rounded p-3 text-xs overflow-auto">
                    {JSON.stringify(sp402Result, null, 2)}
                  </pre>
                )}
              </div>
            </div>

            <div className="card md:col-span-2">
              <h2 className="section-title" style={{marginBottom:"14px"}}>Sandbox Demo Payment</h2>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Endpoint</label>
                  <input
                    className="input-field"
                    value={spDemoEndpoint}
                    onChange={(e) => setSpDemoEndpoint(e.target.value)}
                  />
                </div>
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Budget (lamports)</label>
                  <input
                    className="input-field"
                    type="number"
                    value={spDemoBudget}
                    onChange={(e) => setSpDemoBudget(e.target.value)}
                  />
                </div>
              </div>
              <button className="btn-primary w-full mt-3" onClick={handleDemoPayment}>
                Run Demo Payment
              </button>
              {spDemoResult && (
                <pre className="bg-gray-800 rounded p-3 text-xs overflow-auto mt-3">
                  {JSON.stringify(spDemoResult, null, 2)}
                </pre>
              )}
            </div>
          </div>
        </div>
      )}

      {tab === "pay-demo" && (
        <div>
          <div className="pay-demo-header">
            <h2 className="pay-demo-title">
              <span>⚡</span> Agentic Commerce Demo
            </h2>
            <p className="pay-demo-subtitle">
              Two autonomous agents negotiate, transact, and deliver data on Solana — no human in the loop except one SOL send.
            </p>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-5">
            {/* ── LEFT: Seller (Helius Monitor) ───────────────────────── */}
            <div className={`agent-panel is-seller ${pdSellerState?.is_running ? "is-running" : ""}`}>
              <div className="panel-header">
                <span className={`status-dot ${pdSellerState?.is_running ? "running" : "stopped"}`} />
                <span className="panel-title">Seller Agent</span>
                <span className="ml-auto text-[10px] font-mono text-yellow-500/70 uppercase tracking-widest">
                  helius-monitor
                </span>
              </div>
              <p className="panel-subtitle">
                Watches a devnet wallet for incoming SOL via Helius. On payment, automatically delivers data to buyer.
              </p>

              <div className="space-y-2">
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Recipient address (devnet)</label>
                  <input className="input-field monospace" placeholder="7xKF3rO1jW..."
                    value={pdRecipient} onChange={(e) => setPdRecipient(e.target.value)} disabled={pdSellerCreated} />
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <label className="block text-xs text-gray-500 mb-1">Amount (SOL)</label>
                    <input className="input-field" value={pdAmount}
                      onChange={(e) => setPdAmount(e.target.value)} disabled={pdSellerCreated} />
                  </div>
                  <div>
                    <label className="block text-xs text-gray-500 mb-1">Label</label>
                    <input className="input-field" value={pdLabel}
                      onChange={(e) => setPdLabel(e.target.value)} disabled={pdSellerCreated} />
                  </div>
                </div>
                <div>
                  <label className="block text-xs text-gray-500 mb-1">Helius API key</label>
                  <input className="input-field monospace" value={pdApiKey}
                    onChange={(e) => setPdApiKey(e.target.value)} disabled={pdSellerCreated} />
                </div>
              </div>

              <button className="btn-primary w-full"
                disabled={pdSellerCreated || !pdRecipient.trim()}
                onClick={async () => {
                  await invoke("create_helius_monitor_agent", {
                    id: pdSellerId, recipient: pdRecipient.trim(),
                    amountSol: parseFloat(pdAmount), apiKey: pdApiKey, label: pdLabel || null,
                  });
                  await invoke("start_agent", { id: pdSellerId });
                  setPdSellerCreated(true);
                }}
              >
                {pdSellerCreated ? "✓ Seller Running" : "Create Seller Agent"}
              </button>

              {pdSolanaPayUrl && (
                <div className="url-display">
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-gray-500 uppercase tracking-widest">Solana Pay URL</span>
                    <button className="copy-btn" onClick={() => copyUrl(pdSolanaPayUrl)}>
                      {copiedUrl ? "✓ Copied" : "Copy"}
                    </button>
                  </div>
                  <div className="url-text">{pdSolanaPayUrl}</div>
                </div>
              )}

              <div className="flex-1 flex flex-col min-h-0">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-[10px] text-gray-500 uppercase tracking-widest">Live Actions</span>
                  {pdSellerState?.is_running && (
                    <span className="flex items-center gap-1 text-[10px] text-green-400/70 font-mono">
                      <span className="status-dot running" style={{width:"6px",height:"6px",marginRight:"4px"}} />
                      polling every 10s
                    </span>
                  )}
                </div>
                <div className="terminal-feed h-52 p-1">
                  {(pdSellerState?.actions ?? []).length === 0
                    ? <div className="feed-empty">awaiting agent start…</div>
                    : (pdSellerState?.actions ?? []).slice().reverse().map((a: AgentAction, i: number) => (
                      <div key={i} className="action-row">
                        <span className="text-gray-600 shrink-0 text-[10px] mt-0.5">
                          {new Date(a.timestamp).toLocaleTimeString()}
                        </span>
                        <span className={actionBadgeClass(a.action_type)}>{a.action_type}</span>
                        <span className="text-gray-400 truncate min-w-0" title={a.details}>{a.details}</span>
                        {a.tx_signature && (
                          <span className="text-gray-600 shrink-0 text-[10px] truncate max-w-[80px]" title={a.tx_signature}>
                            {a.tx_signature.slice(0, 8)}…
                          </span>
                        )}
                      </div>
                    ))}
                </div>
              </div>
            </div>

            {/* ── RIGHT: Buyer (Solana Pay Transfer) ──────────────────── */}
            <div className={`agent-panel ${pdBuyerState?.is_running ? "is-running" : ""}`}>
              <div className="panel-header">
                <span className={`status-dot ${pdBuyerState?.is_running ? "running" : "stopped"}`} />
                <span className="panel-title">Buyer Agent</span>
                <span className="ml-auto text-[10px] font-mono text-blue-400/70 uppercase tracking-widest">
                  solana-pay-transfer
                </span>
              </div>
              <p className="panel-subtitle">
                Requests data from seller via the message bus. Receives automatic delivery once payment is confirmed on-chain.
              </p>

              <button className="btn-primary w-full"
                disabled={pdBuyerCreated || !pdSellerCreated}
                onClick={async () => {
                  await invoke("create_solana_pay_agent", { id: pdBuyerId, mode: "Transfer" });
                  await invoke("start_agent", { id: pdBuyerId });
                  await invoke("send_message", {
                    from: pdBuyerId, to: pdSellerId,
                    msgType: "data-request", payload: "requesting AAPL price",
                  });
                  setPdBuyerCreated(true);
                }}
              >
                {pdBuyerCreated ? "✓ Buyer Running" : "Create Buyer Agent"}
              </button>

              {!pdSellerCreated && (
                <p className="text-xs text-yellow-600/80 font-mono">↑ Create the seller agent first</p>
              )}

              {pdBuyerCreated && pdSolanaPayUrl && !pdSaleCompleted && (
                <div className="waiting-payment">
                  <div className="text-yellow-400 font-semibold text-xs flex items-center gap-2">
                    <span className="animate-spin inline-block">◌</span> Waiting for payment
                  </div>
                  <div className="text-gray-400 text-xs">Open Phantom (devnet) and send to:</div>
                  <div className="url-display mt-1">
                    <div className="url-text text-yellow-300">{pdSolanaPayUrl}</div>
                  </div>
                </div>
              )}

              {pdSaleCompleted && (
                <div className="payment-confirmed">
                  <div className="payment-confirmed-title">
                    <span>✓</span> Payment confirmed — data delivered via pay.sh
                  </div>
                  <pre className="payment-confirmed-data">
                    {pdDataResult
                      ? (() => { try { return JSON.stringify(JSON.parse(pdDataResult), null, 2); } catch { return pdDataResult; } })()
                      : "loading…"}
                  </pre>
                </div>
              )}

              <div className="flex-1 flex flex-col min-h-0">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-[10px] text-gray-500 uppercase tracking-widest">Live Actions</span>
                </div>
                <div className="terminal-feed h-52 p-1">
                  {(pdBuyerState?.actions ?? []).length === 0
                    ? <div className="feed-empty">awaiting agent start…</div>
                    : (pdBuyerState?.actions ?? []).slice().reverse().map((a: AgentAction, i: number) => (
                      <div key={i} className="action-row">
                        <span className="text-gray-600 shrink-0 text-[10px] mt-0.5">
                          {new Date(a.timestamp).toLocaleTimeString()}
                        </span>
                        <span className={actionBadgeClass(a.action_type)}>{a.action_type}</span>
                        <span className="text-gray-400 truncate min-w-0" title={a.details}>{a.details}</span>
                      </div>
                    ))}
                </div>
              </div>
            </div>
          </div>

          {/* ── Footer status strip ─────────────────────────────────── */}
          <div className="demo-footer">
            <span className="flex items-center text-gray-500">
              <span className={`demo-footer-dot ${pdSellerState?.is_running ? "active" : "inactive"}`} />
              Seller {pdSellerState?.is_running ? "polling Helius devnet every 10s" : "offline"}
            </span>
            <span className="flex items-center text-gray-500">
              <span className={`demo-footer-dot ${pdBuyerState?.is_running ? "active" : "inactive"}`} />
              Buyer {pdBuyerState?.is_running ? "on message bus" : "offline"}
            </span>
            <span className="flex items-center text-gray-500">
              <span className={`demo-footer-dot ${pdSaleCompleted ? "active" : "inactive"}`} />
              Shared state{pdSaleCompleted ? `: sale/${pdSellerId}/result written` : ": empty"}
            </span>
            <span className="ml-auto text-gray-700">
              CoralOS + Helius + Solana Pay
            </span>
          </div>
        </div>
      )}

      {tab === "payment-flows" && (
        <PaymentFlowsTab
          flows={paymentFlows}
          selectedFlowId={selectedFlowId}
          onSelect={setSelectedFlowId}
        />
      )}

      {tab === "python-agent" && (
        <div className="space-y-5">
          {!IS_TAURI && (
            <div className="card" style={{background:"rgba(245,158,11,0.08)",border:"1px solid rgba(245,158,11,0.3)"}}>
              <p style={{fontSize:"13px",color:"#f59e0b",fontWeight:600,marginBottom:"6px"}}>Tauri Desktop Only</p>
              <p style={{fontSize:"12px",color:"var(--text-dim)"}}>
                The Python side-car agent spawns a subprocess and is only available when running inside Tauri.
                Start the app with <span className="mono">cargo tauri dev</span> to use this feature.
              </p>
            </div>
          )}
          <div className="card space-y-3">
            <div className="flex items-center gap-2">
              <h2 className="section-title" style={{margin:0}}>Python Side-car Agent</h2>
              <span className={`status-dot ${pyRunning ? "running" : "stopped"}`} />
              <span style={{fontSize:"12px",color:"var(--text-dim)"}}>{pyRunning ? "RUNNING" : "STOPPED"}</span>
            </div>
            <p style={{fontSize:"12px",color:"var(--text-dim)"}}>
              Launches <span className="mono">coral-agents/&lt;agent&gt;/agent.py</span> as a
              subprocess and streams its events live. The Helius monitor watches a wallet and
              reports payments.
            </p>

            <div className="flex flex-wrap gap-3">
              <label className="flex flex-col" style={{fontSize:"11px",color:"var(--text-dim)"}}>
                Agent
                <select className="input-field" value={pyAgent} onChange={(e) => setPyAgent(e.target.value)} disabled={pyRunning}>
                  <option value="helius-monitor">helius-monitor</option>
                </select>
              </label>
              <label className="flex flex-col" style={{fontSize:"11px",color:"var(--text-dim)"}}>
                Mode
                <select className="input-field" value={pyMode} onChange={(e) => setPyMode(e.target.value)} disabled={pyRunning}>
                  <option value="standalone">standalone</option>
                  <option value="coral">coral (scaffold)</option>
                </select>
              </label>
              <label className="flex flex-col flex-1" style={{fontSize:"11px",color:"var(--text-dim)",minWidth:"260px"}}>
                Wallet (recipient pubkey)
                <input className="input-field" value={pyWallet} onChange={(e) => setPyWallet(e.target.value)} placeholder="Solana pubkey" disabled={pyRunning} />
              </label>
              <label className="flex flex-col" style={{fontSize:"11px",color:"var(--text-dim)",width:"110px"}}>
                Amount (SOL)
                <input className="input-field" value={pyAmount} onChange={(e) => setPyAmount(e.target.value)} disabled={pyRunning} />
              </label>
            </div>

            <div className="flex flex-wrap gap-3">
              <label className="flex flex-col flex-1" style={{fontSize:"11px",color:"var(--text-dim)",minWidth:"220px"}}>
                Helius API key (optional — builds mainnet URLs)
                <input className="input-field" type="password" value={pyHeliusKey} onChange={(e) => setPyHeliusKey(e.target.value)} disabled={pyRunning} />
              </label>
              <label className="flex flex-col flex-1" style={{fontSize:"11px",color:"var(--text-dim)",minWidth:"220px"}}>
                RPC URL (optional)
                <input className="input-field" value={pyRpcUrl} onChange={(e) => setPyRpcUrl(e.target.value)} placeholder="https://api.devnet.solana.com" disabled={pyRunning} />
              </label>
              <label className="flex flex-col flex-1" style={{fontSize:"11px",color:"var(--text-dim)",minWidth:"220px"}}>
                WS URL (optional)
                <input className="input-field" value={pyWsUrl} onChange={(e) => setPyWsUrl(e.target.value)} placeholder="wss://api.devnet.solana.com" disabled={pyRunning} />
              </label>
            </div>

            <div className="flex items-center gap-3">
              {!pyRunning ? (
                <button className="btn-primary" onClick={startPyAgent}>Start Agent</button>
              ) : (
                <button className="btn-primary" onClick={stopPyAgent} style={{background:"var(--red,#c0392b)"}}>Stop Agent</button>
              )}
              <button className="btn-secondary" onClick={() => setPyEvents([])}>Clear Log</button>
              {pyError && <span style={{fontSize:"12px",color:"var(--red,#e74c3c)"}}>{pyError}</span>}
            </div>
          </div>

          <div className="card">
            <h2 className="section-title" style={{marginBottom:"10px"}}>
              Event Stream <span style={{color:"var(--text-dim)",fontWeight:400}}>({pyEvents.length})</span>
            </h2>
            {pyEvents.length === 0 && <p className="feed-empty">No events yet. Start the agent.</p>}
            <div className="mono" style={{fontSize:"11px",maxHeight:"420px",overflowY:"auto"}}>
              {pyEvents.slice().reverse().map((ev, i) => {
                const color =
                  ev.type === "payment-received" ? "var(--green,#2ecc71)" :
                  ev.type === "error" || ev.type === "stderr" ? "var(--red,#e74c3c)" :
                  ev.type === "partial-payment" ? "var(--yellow,#f1c40f)" :
                  "var(--text-primary)";
                return (
                  <div key={pyEvents.length - i} style={{padding:"3px 0",borderBottom:"1px solid var(--border,#222)"}}>
                    <span style={{color,fontWeight:600}}>{ev.type}</span>
                    <span style={{color:"var(--text-dim)"}}> — {JSON.stringify(
                      Object.fromEntries(Object.entries(ev).filter(([k]) => k !== "type"))
                    )}</span>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}

      {tab === "weather" && (
        <div className="space-y-5">
          <div className="card space-y-4">
            <div>
              <h2 className="section-title" style={{marginBottom:"4px"}}>Weather Agent</h2>
              <p style={{fontSize:"12px",color:"var(--text-dim)"}}>
                Real-time data from open-meteo.com — no API key. Runs the same{" "}
                <span className="mono">WeatherStrategy</span> that backs the web marketplace.
                Tauri calls Rust directly; no coral-server needed.
              </p>
            </div>

            <div className="flex gap-3 items-end">
              <label className="flex flex-col flex-1" style={{fontSize:"11px",color:"var(--text-dim)"}}>
                City
                <input
                  className="input-field"
                  value={weatherCity}
                  onChange={e => setWeatherCity(e.target.value)}
                  onKeyDown={e => e.key === "Enter" && handleWeatherQuery()}
                  placeholder="London, Tokyo, New York…"
                />
              </label>
              <button
                className="btn-primary"
                onClick={handleWeatherQuery}
                disabled={weatherLoading || !weatherCity.trim()}
                style={{minWidth:"120px"}}
              >
                {weatherLoading ? "Fetching…" : "Get Weather"}
              </button>
            </div>

            {weatherError && (
              <div className="card" style={{background:"rgba(231,76,60,0.08)",border:"1px solid rgba(231,76,60,0.3)"}}>
                <p style={{fontSize:"12px",color:"#e74c3c"}}>{weatherError}</p>
              </div>
            )}

            {weatherResult && (
              <div className="card" style={{background:"rgba(30,215,96,0.05)",border:"1px solid rgba(30,215,96,0.2)"}}>
                <div className="flex items-baseline gap-3 mb-3">
                  <span style={{fontSize:"24px",fontWeight:700,color:"var(--green,#2ecc71)"}}>
                    {typeof weatherResult.temperature_c === "number"
                      ? `${weatherResult.temperature_c}°C`
                      : "—"}
                  </span>
                  <span style={{fontSize:"14px",color:"var(--text-dim)"}}>
                    {String(weatherResult.condition ?? "")}
                  </span>
                </div>
                <div className="grid grid-cols-3 gap-3 mb-3" style={{fontSize:"12px"}}>
                  <div>
                    <span style={{color:"var(--text-dim)"}}>City</span>
                    <div style={{fontWeight:600}}>{String(weatherResult.city ?? "")}</div>
                  </div>
                  <div>
                    <span style={{color:"var(--text-dim)"}}>Humidity</span>
                    <div style={{fontWeight:600}}>{weatherResult.humidity_pct}%</div>
                  </div>
                  <div>
                    <span style={{color:"var(--text-dim)"}}>Wind</span>
                    <div style={{fontWeight:600}}>{weatherResult.wind_mph} mph</div>
                  </div>
                </div>
                <details>
                  <summary style={{fontSize:"11px",color:"var(--text-dim)",cursor:"pointer"}}>Raw JSON</summary>
                  <pre className="mono" style={{fontSize:"11px",marginTop:"8px",overflowX:"auto",whiteSpace:"pre-wrap"}}>
                    {JSON.stringify(weatherResult, null, 2)}
                  </pre>
                </details>
              </div>
            )}

            <div className="card" style={{background:"rgba(153,69,255,0.06)",border:"1px solid rgba(153,69,255,0.25)"}}>
              <p style={{fontSize:"12px",color:"var(--text-dim)"}}>
                <strong style={{color:"var(--text-primary)"}}>How the payment rail works:</strong>{" "}
                The web frontend at <span className="mono">localhost:3000</span> sends 0.0005 SOL on
                Solana devnet, then calls <span className="mono">POST /api/v1/weather</span> on
                coral-server. This Tauri tab calls the same Rust strategy natively —
                no HTTP round-trip.
              </p>
            </div>
          </div>
        </div>
      )}

      <footer className="mt-8 text-xs text-gray-600 text-center">
        Built for the Agent Economy Hackathon — CoralOS + Helius + Solana Pay + Multi-Agent Collaboration
      </footer>
    </div>
  );
}

function AgentDetail({ id, state }: { id: string; state: AgentState }) {
  const avgLatency =
    state.actions.length > 0
      ? Math.round(
          state.actions.reduce((a, b) => a + b.latency_ms, 0) / state.actions.length
        )
      : 0;

  return (
    <div className="space-y-5">
      <div className="card">
        <h2 className="section-title" style={{marginBottom:"14px"}}>Agent: {id}</h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
          <div>
            <span className="text-gray-500">Status</span>
            <div className="flex items-center mt-1">
              <span className={`status-dot ${state.is_running ? "running" : "stopped"}`} />
              <span className="font-medium">{state.is_running ? "RUNNING" : "STOPPED"}</span>
            </div>
          </div>
          <div>
            <span className="text-gray-500">Network</span>
            <div className="font-mono text-blue-300 mt-1">{state.network}</div>
          </div>
          <div>
            <span className="text-gray-500">Strategy</span>
            <div className="font-mono text-blue-300 mt-1">{state.strategy}</div>
          </div>
          <div>
            <span className="text-gray-500">Total Actions</span>
            <div className="font-mono text-white mt-1">{state.actions.length}</div>
          </div>
          <div>
            <span className="text-gray-500">Avg Latency</span>
            <div
              className={`font-mono mt-1 ${
                avgLatency < 100 ? "text-green-400" : avgLatency < 500 ? "text-yellow-400" : "text-red-400"
              }`}
            >
              {avgLatency}ms
            </div>
          </div>
          <div>
            <span className="text-gray-500">RPC Endpoint</span>
            <div className="font-mono text-gray-400 mt-1 truncate">{state.rpc_endpoint}</div>
          </div>
        </div>
      </div>

      <div className="card">
        <h2 className="section-title" style={{marginBottom:"14px"}}>Action Log</h2>
        <div className="terminal-feed max-h-[500px] p-1">
          {state.actions.length === 0
            ? <div className="feed-empty">No actions recorded yet.</div>
            : [...state.actions].reverse().map((a, i) => (
              <div key={i} className="action-row">
                <span className="text-gray-600 shrink-0 text-[10px] mt-0.5 w-16">
                  {new Date(a.timestamp).toLocaleTimeString()}
                </span>
                <span className={`action-badge ${
                  a.action_type.includes("error") ? "type-rpc-error" :
                  a.action_type === "payment-received" ? "type-payment-received" :
                  a.action_type === "data-delivered" ? "type-data-delivered" :
                  a.action_type === "data-received"  ? "type-data-received"  :
                  a.action_type === "url-generated"  ? "type-url-generated"  :
                  a.action_type === "poll-tick"       ? "type-poll-tick"       :
                  "type-default"
                }`}>{a.action_type}</span>
                <span className="text-gray-400 truncate min-w-0 flex-1" title={a.details}>{a.details}</span>
                {a.slot && <span className="text-gray-600 shrink-0 text-[10px]">#{a.slot}</span>}
                {a.latency_ms > 0 && (
                  <span className={`shrink-0 text-[10px] font-mono ${
                    a.latency_ms < 100 ? "text-green-500" :
                    a.latency_ms < 500 ? "text-yellow-500" : "text-red-500"
                  }`}>{a.latency_ms}ms</span>
                )}
              </div>
            ))}
        </div>
      </div>
    </div>
  );
}

function CoralSessionDetail({ session }: { session: CoralSessionExtended }) {
  return (
    <div className="space-y-5">
      <div className="card">
        <h2 className="section-title" style={{marginBottom:"14px"}}>Session: {session.id.slice(0, 16)}...</h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
          <div>
            <span className="text-gray-500">Status</span>
            <div className="flex items-center mt-1">
              <span
                className={`status-dot ${
                  session.status === "running" ? "running" : "stopped"
                }`}
              />
              <span className="font-medium">{session.status.toUpperCase()}</span>
            </div>
          </div>
          <div>
            <span className="text-gray-500">Namespace</span>
            <div className="font-mono text-blue-300 mt-1">{session.namespace}</div>
          </div>
          <div>
            <span className="text-gray-500">Agents</span>
            <div className="font-mono text-white mt-1">{session.agents?.length ?? 0}</div>
          </div>
          <div>
            <span className="text-gray-500">Payment Session</span>
            <div className="font-mono text-gray-400 mt-1 truncate">
              {session.paymentSessionId ?? "None"}
            </div>
          </div>
        </div>
      </div>

      <div className="card">
        <h2 className="section-title" style={{marginBottom:"14px"}}>Agents in Session</h2>
        {session.agents?.length === 0 && (
          <p className="feed-empty">No agents in this session.</p>
        )}
        <div className="space-y-3">
          {session.agents?.map((agent, i) => (
            <div key={i} className="bg-gray-800/50 rounded p-3 border border-gray-700/50">
              <div className="flex items-center justify-between mb-1">
                <span className="font-mono text-sm font-semibold text-white">{agent.name}</span>
                <span
                  className={`inline-block px-2 py-0.5 rounded text-xs font-semibold ${
                    agent.status.includes("running")
                      ? "bg-green-900/50 text-green-300"
                      : agent.status.includes("waiting")
                      ? "bg-yellow-900/50 text-yellow-300"
                      : "bg-gray-700 text-gray-300"
                  }`}
                >
                  {agent.status}
                </span>
              </div>
              <div className="text-xs text-gray-400 truncate" title={agent.description}>
                {agent.description}
              </div>
              {agent.links.length > 0 && (
                <div className="mt-2 text-xs text-gray-500">Links: {agent.links.length}</div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function WorkflowDetail({
  workflow,
  agents,
  onAssign,
  onStart,
  onComplete,
}: {
  workflow: Workflow;
  agents: string[];
  onAssign: (wfId: string, stepId: string, agentId: string) => void;
  onStart: (wfId: string, stepId: string) => void;
  onComplete: (wfId: string, stepId: string) => void;
}) {
  const statusColor = (status: string) => {
    switch (status) {
      case "Completed":
        return "bg-green-900/50 text-green-300";
      case "Failed":
        return "bg-red-900/50 text-red-300";
      case "InProgress":
        return "bg-blue-900/50 text-blue-300";
      case "Assigned":
        return "bg-yellow-900/50 text-yellow-300";
      default:
        return "bg-gray-700 text-gray-300";
    }
  };

  return (
    <div className="space-y-5">
      <div className="card">
        <h2 className="section-title" style={{marginBottom:"14px"}}>{workflow.name}</h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
          <div>
            <span className="text-gray-500">Status</span>
            <div className="mt-1">
              <span
                className={`inline-block px-2 py-0.5 rounded text-xs font-semibold ${statusColor(
                  workflow.status
                )}`}
              >
                {workflow.status}
              </span>
            </div>
          </div>
          <div>
            <span className="text-gray-500">Priority</span>
            <div className="font-mono text-white mt-1">{workflow.priority}/10</div>
          </div>
          <div>
            <span className="text-gray-500">Steps</span>
            <div className="font-mono text-white mt-1">{workflow.steps.length}</div>
          </div>
          <div>
            <span className="text-gray-500">Progress</span>
            <div className="font-mono text-white mt-1">
              {workflow.steps.filter((s) => s.status === "Completed").length} / {workflow.steps.length}
            </div>
          </div>
        </div>
      </div>

      <div className="card">
        <h2 className="section-title" style={{marginBottom:"14px"}}>Steps</h2>
        <div className="space-y-3">
          {workflow.steps.map((step, i) => (
            <div key={step.id} className="bg-gray-800/50 rounded p-3 border border-gray-700/50">
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <span className="text-xs text-gray-500">#{i + 1}</span>
                  <span className="font-mono text-sm font-semibold text-white">{step.name}</span>
                  <span
                    className={`inline-block px-2 py-0.5 rounded text-xs font-semibold ${statusColor(
                      step.status
                    )}`}
                  >
                    {step.status}
                  </span>
                </div>
                {step.assigned_to && (
                  <span className="text-xs text-gray-400">Assigned: {step.assigned_to}</span>
                )}
              </div>
              <div className="text-xs text-gray-400 mb-2">{step.description}</div>
              <div className="flex gap-2 flex-wrap">
                {step.status === "Pending" && (
                  <>
                    <select
                      className="input-field monospace" style={{width:"auto",fontSize:"11px",padding:"3px 24px 3px 7px"}}
                      onChange={(e) => {
                        if (e.target.value) onAssign(workflow.id, step.id, e.target.value);
                      }}
                    >
                      <option value="">Assign to...</option>
                      {agents.map((a) => (
                        <option key={a} value={a}>
                          {a}
                        </option>
                      ))}
                    </select>
                    {step.assigned_to && (
                      <button
                        className="btn-primary text-xs py-1 px-2"
                        onClick={() => onStart(workflow.id, step.id)}
                      >
                        Start
                      </button>
                    )}
                  </>
                )}
                {step.status === "InProgress" && (
                  <button
                    className="btn-primary text-xs py-1 px-2"
                    onClick={() => onComplete(workflow.id, step.id)}
                  >
                    Complete
                  </button>
                )}
                {step.result && (
                  <span className="text-xs text-gray-400">Result: {step.result}</span>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// ── Payment Flows Debugger ───────────────────────────────────────────────────

function protocolBadge(protocol: string | null) {
  if (!protocol) return "bg-gray-700 text-gray-400";
  return protocol === "mpp" ? "bg-purple-900/60 text-purple-300" : "bg-cyan-900/60 text-cyan-300";
}

function flowStatusBadge(status: string) {
  switch (status) {
    case "success": return "bg-green-900/60 text-green-300";
    case "failed": return "bg-red-900/60 text-red-300";
    default: return "bg-gray-700 text-gray-400";
  }
}

function SequenceStep({
  label,
  sublabel,
  done,
  active,
}: {
  label: string;
  sublabel: string;
  done: boolean;
  active: boolean;
}) {
  return (
    <div className={`flex flex-col items-center gap-1 flex-1 ${done ? "opacity-100" : "opacity-30"}`}>
      <div
        className={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold border-2 ${
          done
            ? active
              ? "border-green-400 bg-green-900/50 text-green-300"
              : "border-blue-500 bg-blue-900/50 text-blue-300"
            : "border-gray-700 bg-gray-900 text-gray-600"
        }`}
      >
        {done ? "✓" : "○"}
      </div>
      <span className="text-[10px] font-semibold text-center leading-tight text-gray-300">{label}</span>
      <span className="text-[9px] text-gray-600 text-center leading-tight">{sublabel}</span>
    </div>
  );
}

function SequenceDiagram({ flow }: { flow: PaymentFlowRecord }) {
  const hasChallenge = !!flow.challenge_at;
  const hasPayment = !!flow.payment_at;
  const hasDelivery = !!flow.delivery_at;

  return (
    <div className="flex items-start gap-0 w-full">
      <SequenceStep label="Request" sublabel="GET endpoint" done={true} active={false} />
      <div className={`flex-1 h-0.5 mt-4 ${hasChallenge ? "bg-blue-600" : "bg-gray-800"}`} />
      <SequenceStep label="402 Challenge" sublabel={flow.protocol?.toUpperCase() ?? "—"} done={hasChallenge} active={false} />
      <div className={`flex-1 h-0.5 mt-4 ${hasPayment ? "bg-blue-600" : "bg-gray-800"}`} />
      <SequenceStep label="Payment Sent" sublabel={flow.token ?? "—"} done={hasPayment} active={false} />
      <div className={`flex-1 h-0.5 mt-4 ${hasDelivery ? "bg-green-600" : "bg-gray-800"}`} />
      <SequenceStep
        label="Delivered"
        sublabel={flow.status === "success" ? "200 OK" : flow.error ? "Error" : "—"}
        done={hasDelivery}
        active={hasDelivery && flow.status === "success"}
      />
    </div>
  );
}

function PaymentFlowDetail({ flow }: { flow: PaymentFlowRecord }) {
  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="card">
        <div className="flex items-center gap-3 mb-3 flex-wrap">
          <span className={`inline-block px-2 py-0.5 rounded text-xs font-semibold ${flowStatusBadge(flow.status)}`}>
            {flow.status.toUpperCase()}
          </span>
          {flow.protocol && (
            <span className={`inline-block px-2 py-0.5 rounded text-xs font-semibold ${protocolBadge(flow.protocol)}`}>
              {flow.protocol.toUpperCase()}
            </span>
          )}
          <span className="font-mono text-xs text-gray-400 truncate">{flow.endpoint}</span>
          <span className="ml-auto text-[10px] text-gray-600 font-mono">{flow.agent_id}</span>
        </div>
        <SequenceDiagram flow={flow} />
      </div>

      {/* Challenge Details */}
      {(flow.amount !== null || flow.recipient || flow.token) && (
        <div className="card">
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-widest mb-3">402 Challenge</h3>
          <div className="grid grid-cols-2 md:grid-cols-3 gap-3 text-sm">
            {flow.amount !== null && (
              <div>
                <div className="text-gray-500 text-xs">Amount</div>
                <div className="font-mono text-white">{flow.amount.toLocaleString()} <span className="text-gray-500 text-xs">lamports</span></div>
              </div>
            )}
            {flow.recipient && (
              <div>
                <div className="text-gray-500 text-xs">Recipient</div>
                <div className="font-mono text-blue-300 text-xs truncate" title={flow.recipient}>{flow.recipient}</div>
              </div>
            )}
            {flow.token && (
              <div>
                <div className="text-gray-500 text-xs">Token</div>
                <div className="font-mono text-yellow-300">{flow.token}</div>
              </div>
            )}
          </div>
          {flow.payment_header && (
            <div className="mt-3">
              <div className="text-gray-500 text-xs mb-1">Payment Header</div>
              <div className="bg-gray-900 rounded p-2 font-mono text-[10px] text-gray-300 break-all">{flow.payment_header}</div>
            </div>
          )}
        </div>
      )}

      {/* Timeline */}
      <div className="card">
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-widest mb-3">Event Timeline</h3>
        <div className="space-y-2">
          {[
            { label: "Request Sent", ts: flow.request_at, color: "text-gray-300" },
            { label: "402 Challenge Received", ts: flow.challenge_at, color: "text-purple-300" },
            { label: "Payment Header Sent", ts: flow.payment_at, color: "text-blue-300" },
            { label: "Resource Delivered", ts: flow.delivery_at, color: "text-green-300" },
          ]
            .filter((e) => e.ts)
            .map((e, i) => (
              <div key={i} className="flex items-center gap-3 text-xs">
                <span className="w-1.5 h-1.5 rounded-full bg-current shrink-0" style={{ color: "inherit" }} />
                <span className={`${e.color} font-medium w-44 shrink-0`}>{e.label}</span>
                <span className="text-gray-500 font-mono">{new Date(e.ts!).toLocaleTimeString()}</span>
              </div>
            ))}
        </div>
      </div>

      {/* Response Body */}
      {flow.response_body && (
        <div className="card">
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-widest mb-2">Response Body</h3>
          <pre className="bg-gray-900 rounded p-3 text-xs text-green-300 overflow-auto max-h-48 font-mono">
            {(() => { try { return JSON.stringify(JSON.parse(flow.response_body), null, 2); } catch { return flow.response_body; } })()}
          </pre>
        </div>
      )}

      {/* Error */}
      {flow.error && (
        <div className="card border border-red-900/50">
          <h3 className="text-xs font-semibold text-red-400 uppercase tracking-widest mb-2">Error</h3>
          <div className="text-xs text-red-300 font-mono">{flow.error}</div>
        </div>
      )}
    </div>
  );
}

function PaymentFlowsTab({
  flows,
  selectedFlowId,
  onSelect,
}: {
  flows: PaymentFlowRecord[];
  selectedFlowId: string | null;
  onSelect: (id: string) => void;
}) {
  const selectedFlow = flows.find((f) => f.id === selectedFlowId) ?? flows[flows.length - 1] ?? null;

  return (
    <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
      {/* Flow List */}
      <div className="lg:col-span-1 space-y-2">
        <h2 className="section-title">
          Payment Flows <span style={{color:"var(--text-dim)",fontWeight:400,fontSize:"13px"}}>({flows.length})</span>
        </h2>
        {flows.length === 0 && (
          <div className="card text-sm text-gray-500">
            No flows recorded yet. Run a Pay Demo or use the x402 Demo Payment in the Solana Pay tab.
          </div>
        )}
        {flows.slice().reverse().map((flow) => (
          <div
            key={flow.id}
            className={`card cursor-pointer transition ${
              (selectedFlowId === flow.id || (!selectedFlowId && flow === flows[flows.length - 1]))
                ? "border-blue-500"
                : "hover:border-gray-600"
            }`}
            onClick={() => onSelect(flow.id)}
          >
            <div className="flex items-center gap-2 mb-1 flex-wrap">
              <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-semibold ${flowStatusBadge(flow.status)}`}>
                {flow.status}
              </span>
              {flow.protocol && (
                <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-semibold ${protocolBadge(flow.protocol)}`}>
                  {flow.protocol}
                </span>
              )}
              <span className="ml-auto text-[10px] text-gray-600 font-mono">{flow.agent_id}</span>
            </div>
            <div className="text-xs text-gray-400 truncate" title={flow.endpoint}>{flow.endpoint}</div>
            <div className="text-[10px] text-gray-600 mt-1 font-mono">
              {new Date(flow.request_at).toLocaleTimeString()}
            </div>
            {/* Mini step indicators */}
            <div className="flex gap-1 mt-2">
              {[
                { label: "REQ", done: true },
                { label: "402", done: !!flow.challenge_at },
                { label: "PAY", done: !!flow.payment_at },
                { label: "DEL", done: !!flow.delivery_at },
              ].map((s) => (
                <span
                  key={s.label}
                  className={`text-[9px] px-1 rounded font-mono ${
                    s.done ? "bg-blue-900/60 text-blue-300" : "bg-gray-800 text-gray-600"
                  }`}
                >
                  {s.label}
                </span>
              ))}
            </div>
          </div>
        ))}
      </div>

      {/* Flow Detail */}
      <div className="lg:col-span-2">
        {selectedFlow ? (
          <PaymentFlowDetail flow={selectedFlow} />
        ) : (
          <div className="card text-sm text-gray-500">Select a flow to inspect it.</div>
        )}
      </div>
    </div>
  );
}

export default App;
