import { Button, Input } from "antd";
import {
  ArrowUpOutlined,
  PauseOutlined,
  CopyOutlined,
} from "@ant-design/icons";
import { useId, useRef, useState, type ReactNode } from "react";
import type { MessageItem } from "../types";
import { MarkdownContent } from "./MarkdownContent";

export function MessageComposer({
  value,
  onChange,
  onSend,
  onCancel,
  disabled = false,
  canSend = true,
  busy = false,
  context,
}: {
  value: string;
  onChange(value: string): void;
  onSend(): void;
  onCancel?(): void;
  disabled?: boolean;
  canSend?: boolean;
  busy?: boolean;
  context?: ReactNode;
}) {
  const composing = useRef(false);
  const hintId = useId();
  const send = () => {
    if (canSend && !disabled && !busy && value.trim()) onSend();
  };
  return (
    <div className="rss-composer-wrap">
      {context}
      <div className="rss-composer">
        <Input.TextArea
          aria-label="消息"
          aria-describedby={hintId}
          value={value}
          disabled={disabled}
          autoSize={{ minRows: 2, maxRows: 8 }}
          onChange={(event) => onChange(event.target.value)}
          onCompositionStart={() => {
            composing.current = true;
          }}
          onCompositionEnd={() => {
            composing.current = false;
          }}
          onKeyDown={(event) => {
            if (
              event.key === "Enter" &&
              !event.shiftKey &&
              !event.nativeEvent.isComposing &&
              !composing.current &&
              event.keyCode !== 229
            ) {
              event.preventDefault();
              send();
            }
          }}
        />
        <div className="rss-composer-bottom">
          <span id={hintId}>Enter 发送 · Shift + Enter 换行</span>
          {busy && onCancel ? (
            <Button
              aria-label="请求取消"
              icon={<PauseOutlined />}
              onClick={onCancel}
            />
          ) : (
            <Button
              type="primary"
              aria-label="发送消息"
              icon={<ArrowUpOutlined />}
              disabled={!canSend || disabled || busy || !value.trim()}
              onClick={send}
            />
          )}
        </div>
      </div>
    </div>
  );
}
export function MessageEntry({
  message,
  onCopy,
}: {
  message: MessageItem;
  onCopy?(text: string): void | Promise<boolean>;
}) {
  const [copyState, setCopied] = useState({ text: "", status: "" }),
    copied = copyState.text === message.text ? copyState.status : "";
  const stable =
    message.kind === "user" || (message.kind === "assistant" && message.stable);
  return (
    <article
      className={`rss-message rss-message-${message.kind}`}
      data-message-key={message.id}
      data-message-kind={message.kind}
      data-stable={stable ? "true" : "false"}
    >
      <div className="rss-message-label">
        {message.kind === "user"
          ? "你"
          : message.kind === "reasoning"
            ? "思考过程"
            : "AI 助手"}
      </div>
      {message.kind === "user" ? (
        <p>{message.text}</p>
      ) : (
        <MarkdownContent
          text={message.text}
          stable={message.kind === "assistant" && message.stable}
          onCopy={
            onCopy
              ? (text) => {
                  void onCopy(text);
                }
              : undefined
          }
        />
      )}
      {stable && onCopy && (
        <div className="rss-message-actions">
          <Button
            type="text"
            size="small"
            icon={<CopyOutlined />}
            aria-label="复制这条消息"
            onClick={async () => {
              try {
                const result = await onCopy(message.text);
                if (typeof result === "boolean")
                  setCopied({
                    text: message.text,
                    status: result ? "已复制" : "复制未完成",
                  });
              } catch {
                setCopied({ text: message.text, status: "复制未完成" });
              }
            }}
          >
            复制
          </Button>
          <span role="status">{copied}</span>
        </div>
      )}
    </article>
  );
}
export function MessageStream({
  messages,
  onCopy,
}: {
  messages: readonly MessageItem[];
  onCopy?(text: string): void | Promise<boolean>;
}) {
  return (
    <div className="rss-messages" role="log" aria-label="对话消息">
      {messages.map((message) => (
        <MessageEntry key={message.id} message={message} onCopy={onCopy} />
      ))}
    </div>
  );
}
