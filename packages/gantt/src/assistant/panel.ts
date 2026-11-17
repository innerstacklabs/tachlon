// The assistant's chat panel (gantt#1875): a message list, an input, and Accept and Reject while a proposal is held. Text only. Each message runs one `runAssistantTurn`; the host's `send` is its own call to its model.
import type { Commands } from "../commands.js";
import { ASSISTANT_EN } from "../texts-en.ts"; // `.ts`, as `loop.ts` does
import "../styles/assistant.css";
import { runAssistantTurn, type Message, type Send } from "./loop.ts";

/** A panel written into the page: `destroy` takes it out again. */
export interface AssistantPanel {
  destroy(): void;
}

function make<K extends keyof HTMLElementTagNameMap>(tag: K, attributes: Record<string, string> = {}, text = ""): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) el.setAttribute(name, value);
  el.textContent = text;
  return el;
}

const held = (messages: readonly Message[]): boolean => messages.some((m) => m.role === "tool" && m.name === "propose" && m.result.ok);

/** Write the panel into `parent`: one turn per message sent, the model's words in a polite live region, and Accept and Reject for the proposal a turn leaves held. Every word is a key of `ASSISTANT_EN`, replaced by the same key in `texts`. */
export function mountAssistantPanel(parent: HTMLElement, send: Send, commands: Commands, texts?: Readonly<Record<string, string>>): AssistantPanel {
  const t = (key: string): string => texts?.[key] ?? ASSISTANT_EN[key] ?? key;
  const panel = make("section", { "data-part": "assistant", "aria-label": t("assistant") });
  const log = make("div", { role: "log", "aria-live": "polite", "aria-label": t("assistant-log") });
  const form = make("form");
  const input = make("input", { type: "text", "aria-label": t("assistant-message"), autocomplete: "off" });
  const sendButton = make("button", { type: "submit" }, t("assistant-send"));
  const actions = make("div", { "data-part": "assistant-actions" });
  const accept = make("button", { type: "button" }, t("assistant-accept"));
  const reject = make("button", { type: "button" }, t("assistant-reject"));
  actions.hidden = true;
  form.append(input, sendButton);
  actions.append(accept, reject);
  panel.append(log, form, actions);
  const scope = parent.closest(".gantt") === null ? make("div", { class: "gantt" }) : parent; // assistant.css and the tokens key on `.gantt`: outside one, the panel brings its own box, which lays out nothing
  if (scope !== parent) { scope.style.display = "contents"; parent.append(scope); }
  scope.append(panel);

  let history: readonly Message[] = [];
  let seen: readonly Message[] = []; // what the model was last handed: the turn so far when `send` rejects
  const say = (role: string, words: string): void => void log.append(make("p", { "data-role": role }, words));
  const spy: Send = (messages) => ((seen = messages), send(messages));

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const words = input.value.trim();
    if (words === "" || sendButton.disabled) return;
    input.value = "";
    say("user", words);
    sendButton.disabled = true;
    const before = history.length + 1;
    runAssistantTurn(spy, [...history, { role: "user", content: words }], commands)
      .then(
        (turn) => {
          history = turn.messages;
          for (const m of turn.messages.slice(before)) {
            if (m.role === "assistant" && m.content !== "") say("assistant", m.content);
          }
          if (held(turn.messages.slice(before))) actions.hidden = false;
          if (turn.stopped === "limit") say("assistant", t("assistant-limit"));
        },
        () => {
          say("assistant", t("assistant-failed"));
          if (held(seen.slice(before))) actions.hidden = false;
        },
      )
      .finally(() => {
        sendButton.disabled = false;
      });
  });
  accept.addEventListener("click", () => {
    const result = commands.acceptProposal();
    if (result?.ok || (result && result.reason === "noProposal")) actions.hidden = true;
    say("assistant", result?.ok ? t("assistant-accepted") : t("assistant-refused"));
  });
  reject.addEventListener("click", () => {
    commands.rejectProposal();
    actions.hidden = true;
    say("assistant", t("assistant-rejected"));
  });
  return { destroy: () => (scope === parent ? panel : scope).remove() };
}
