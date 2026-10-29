import type { Snapshot } from "valtio/vanilla";
import type { SessionView } from "@rss-mdm-agent/ai-client";
import type { MessageItem } from "@rss-mdm-agent/ui";
/** The existing client owns sequence and certainty; this function supplies display nodes only. */
export function conversationMessages(
  view?: Snapshot<SessionView>,
): MessageItem[] {
  if (!view) return [];
  return view.timeline.flatMap<MessageItem>((item) => {
    if (item.kind === "prompt") {
      const command = view.commands[item.key]?.command;
      return command?.input.type === "prompt"
        ? [{ id: item.key, kind: "user", text: command.input.text }]
        : [];
    }
    if (item.kind === "message") {
      const message = view.messages[item.key];
      return message
        ? [
            {
              id: item.key,
              kind: "assistant",
              text: message.text,
              stable: message.stable,
            },
          ]
        : [];
    }
    return [];
  });
}
