<script setup lang="ts">
import { Monitor, Package, Sparkles } from "@rss-mdm-agent/ui";
import WindowControls from "../workspace/WindowControls.vue";
import Account from "../settings/Account.vue";
import type { EnterpriseLogin, FixtureLoginPort } from "./types";
import type { UserContext } from "@rss-mdm-agent/ai-contract/wire";
defineProps<{
  loading: boolean;
  message: string;
  fixture?: FixtureLoginPort;
}>();
const emit = defineEmits<{
  login: [input: EnterpriseLogin];
  guest: [];
  recover: [clear: boolean];
  authenticated: [context: UserContext];
  settings: [];
  back: [];
}>();
</script>
<template>
  <section class="login-screen" aria-label="登录工作空间">
    <header class="login-window" data-tauri-drag-region>
      <span>RSS Workspace</span><WindowControls />
    </header>
    <div class="login-columns">
      <aside class="login-story" aria-label="RSS Workspace">
        <div class="login-brand">
          <i>r.</i><strong>RSS</strong><small>Workspace</small>
        </div>
        <div class="story-copy">
          <span>● YOUR EVERYDAY COMPANION</span>
          <h1>好工具，<br />让工作轻一点。</h1>
          <p>
            常用应用、设备服务与 AI 助手。<br />一个工作空间，照顾工作的每一天。
          </p>
        </div>
        <div class="login-art" aria-hidden="true">
          <div class="art-orbit" />
          <span class="art-tile first"><Package :size="25" /></span
          ><span class="art-tile second"><Sparkles :size="25" /></span>
          <div class="art-laptop">
            <div class="art-screen">
              <div class="art-sidebar"><strong>r.</strong><i /><i /><i /></div>
              <div class="art-desktop">
                <small>YOUR EVERYDAY COMPANION</small>
                <div>
                  <strong>好状态，开启好的一天。</strong>
                  <p>你的电脑与工作，好好相处。</p>
                  <Monitor :size="26" />
                </div>
                <footer><i /><i /><i /></footer>
              </div>
            </div>
            <div class="art-base" />
          </div>
          <span class="art-note">✓ 为你的每一天，准备就绪</span>
        </div>
        <footer class="story-footer">
          安心连接，专注眼前。<span>━━ · ·</span>
        </footer>
      </aside>
      <div class="login-panel">
        <div class="login-tools">
          <button
            class="login-diagnostics"
            :disabled="loading"
            @click="emit('settings')"
          >
            设置与诊断</button
          ><slot name="back" />
        </div>
        <div class="login-form">
          <div class="mobile-brand login-brand">
            <i>r.</i><strong>RSS</strong><small>Workspace</small>
          </div>
          <Account
            :loading="loading"
            :message="message"
            :fixture="fixture"
            @login="emit('login', $event)"
            @guest="emit('guest')"
            @recover="emit('recover', $event)"
            @authenticated="emit('authenticated', $event)"
            ><template #qr><slot name="login-qr" /></template
          ></Account>
        </div>
        <p class="security-note">你的组织账号，由组织安全管理</p>
      </div>
    </div>
  </section>
</template>
<style scoped>
.login-screen {
  height: 100%;
  min-height: 0;
  background: var(--rss-color-surface);
  color: var(--rss-color-text);
  display: flex;
  flex-direction: column;
}
.login-window {
  height: 42px;
  flex: none;
  border-bottom: 1px solid var(--rss-color-border);
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  color: var(--rss-color-text-muted);
  font-size: 11px;
}
.login-window :deep(.window-controls) {
  position: absolute;
  right: 8px;
  top: 8px;
  bottom: 8px;
}
.login-columns {
  display: grid;
  grid-template-columns: 1fr 1fr;
  flex: 1;
  min-height: 0;
}
.login-story {
  margin: 16px 0 16px 16px;
  border-radius: 12px;
  padding: 38px 40px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
  isolation: isolate;
  color: #32483d;
  background:
    radial-gradient(ellipse at 85% 80%, #f5f1e1, transparent 60%),
    radial-gradient(ellipse at 10% 20%, #dbe7dd, transparent 75%), #e9eeea;
}
.login-brand {
  display: flex;
  align-items: center;
  gap: 10px;
}
.login-brand i {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  background: #293d32;
  color: #fff;
  border-radius: 10px;
  font-size: 25px;
  font-style: normal;
  font-weight: 700;
  letter-spacing: -3px;
  padding-right: 3px;
  line-height: 1;
}
.login-brand strong {
  font-size: 16px;
  font-weight: 600;
}
.login-brand small {
  font-size: 11px;
  opacity: 0.6;
}
.story-copy {
  margin-top: 58px;
  z-index: 2;
}
.story-copy > span {
  font-size: 9px;
  letter-spacing: 2px;
  font-weight: 550;
}
.story-copy h1 {
  font-size: 42px;
  line-height: 1.42;
  letter-spacing: -1.7px;
  font-weight: 600;
  margin: 21px 0 18px;
}
.story-copy p {
  font-size: 13px;
  line-height: 1.95;
  color: #6e8275;
  margin: 0;
}
.login-art {
  position: relative;
  min-height: 240px;
  flex: 1;
  display: grid;
  place-items: center;
  perspective: 1000px;
  margin-top: 15px;
}
.art-orbit {
  position: absolute;
  width: 345px;
  height: 220px;
  border: 1px solid #73968120;
  border-radius: 50%;
  transform: rotate(-22deg);
}
.art-orbit:after {
  content: "";
  position: absolute;
  inset: -20px;
  border: 1px solid #73968118;
  border-radius: 50%;
  transform: rotate(40deg);
}
.art-laptop {
  width: 268px;
  transform: rotate(-8deg) rotateY(-10deg);
  filter: drop-shadow(5px 26px 18px #3a564424);
  z-index: 2;
}
.art-screen {
  height: 165px;
  background: #fdfefc;
  border: 6px solid #526a5b;
  border-radius: 10px;
  display: flex;
  overflow: hidden;
}
.art-sidebar {
  width: 44px;
  background: #f1f4ef;
  padding: 10px 7px;
  color: #57705c;
}
.art-sidebar strong {
  font-size: 15px;
}
.art-sidebar i {
  display: block;
  height: 4px;
  width: 25px;
  margin: 10px 0;
  border-radius: 3px;
  background: #d8e1d5;
}
.art-desktop {
  padding: 14px 12px;
  width: calc(100% - 44px);
}
.art-desktop > small {
  display: block;
  font-size: 6px;
  color: #627568;
  margin-bottom: 8px;
}
.art-desktop > div {
  position: relative;
  background: linear-gradient(120deg, #e0e9dd, #f1eee1);
  border-radius: 5px;
  padding: 11px;
  color: #53705b;
}
.art-desktop strong {
  font-size: 9px;
  font-weight: 600;
}
.art-desktop p {
  font-size: 5px;
  margin: 4px 0;
}
.art-desktop :deep(svg) {
  position: absolute;
  right: 8px;
  bottom: 9px;
  opacity: 0.4;
}
.art-desktop > footer {
  display: flex;
  gap: 6px;
  margin-top: 9px;
}
.art-desktop > footer i {
  width: 50px;
  height: 30px;
  background: #f2f4f0;
  border: 1px solid #e6ece4;
  border-radius: 4px;
}
.art-base {
  width: 295px;
  height: 8px;
  margin-left: -13px;
  background: linear-gradient(#ccd5c8, #a9b8a5);
  border-radius: 2px 2px 9px 9px;
}
.art-tile {
  position: absolute;
  display: grid;
  place-items: center;
  width: 49px;
  height: 49px;
  border-radius: 14px;
  background: #fffffff0;
  box-shadow: 0 9px 20px #426b4412;
  border: 1px solid white;
  z-index: 3;
  color: #789682;
}
.art-tile.first {
  top: 24%;
  left: 4%;
  transform: rotate(-12deg);
}
.art-tile.second {
  top: 12%;
  right: 7%;
  transform: rotate(12deg);
  color: #a48f5b;
  background: #fcfaf2;
}
.art-note {
  position: absolute;
  bottom: 6%;
  left: 6%;
  font-size: 9px;
  padding: 8px 12px;
  background: #ffffffdf;
  border: 1px solid white;
  border-radius: 30px;
  color: #627968;
  z-index: 3;
}
.story-footer {
  display: flex;
  justify-content: space-between;
  font-size: 10px;
  color: #6e8275;
  padding-top: 16px;
}
.login-panel {
  min-width: 0;
  overflow: auto;
  scrollbar-gutter: stable;
  display: flex;
  flex-direction: column;
  padding: 20px 40px 18px;
}
.login-tools {
  height: 22px;
  min-height: 22px;
  display: flex;
  justify-content: flex-end;
  gap: 15px;
}
.login-tools button {
  font: inherit;
  border: 0;
  background: none;
  color: var(--rss-color-text-muted);
  font-size: 11px;
  cursor: pointer;
}
.login-tools button:focus-visible {
  outline: 2px solid var(--rss-color-accent);
  outline-offset: 3px;
}
.login-form {
  height: 626px;
  min-height: 626px;
  width: 100%;
  max-width: 348px;
  margin: auto;
}
.security-note {
  min-height: 22px;
  text-align: center;
  font-size: 10px;
  color: var(--rss-color-text-muted);
  margin: 0;
}
.mobile-brand {
  display: none;
}
@media (max-width: 1100px) {
  .login-story {
    padding: 30px 26px;
  }
  .story-copy h1 {
    font-size: 36px;
  }
  .login-panel {
    padding: 18px 26px;
  }
  .story-copy {
    margin-top: 45px;
  }
}
@media (max-width: 760px) {
  .login-columns {
    grid-template-columns: 1fr;
  }
  .login-story {
    display: none;
  }
  .login-panel {
    padding: 16px 28px 15px;
  }
  .login-form {
    height: 694px;
    min-height: 694px;
    margin: 10px auto;
  }
  .mobile-brand {
    display: flex;
    height: 32px;
    margin-bottom: 25px;
  }
}
@media (max-height: 760px) and (min-width: 761px) {
  .login-panel {
    padding-top: 12px;
    padding-bottom: 10px;
  }
  .story-copy {
    margin-top: 30px;
  }
  .story-copy h1 {
    font-size: 34px;
  }
  .login-art {
    min-height: 200px;
  }
}
</style>
