<script setup lang="ts">
defineProps<{ kind: string }>();
</script>
<template>
  <div class="app-artwork" :class="kind" aria-hidden="true">
    <template v-if="kind === 'notes' || kind === 'pdf'"
      ><div class="paper"><span>A little idea.</span><i /><i /><i /><i /></div>
      <small>Ideas worth keeping</small></template
    ><template v-else-if="kind === 'design'"
      ><div class="design-board"><i /><span>Aa</span></div>
      <div class="swatches"><i /><i /><i /></div></template
    ><template v-else-if="kind === 'browser'"
      ><div class="browser-window">
        <header>••• <i /></header>
        <div><span /><i /></div></div></template
    ><template v-else
      ><div class="workspace-window">
        <div class="art-side"><i /><i /><i /><i /></div>
        <div class="art-main"><i /><i /><i /><i /></div>
      </div>
      <small>让好想法，在一起</small></template
    >
  </div>
</template>
<style scoped>
.app-artwork {
  position: relative;
  height: 180px;
  overflow: hidden;
  display: grid;
  place-items: center;
  background: #edf0e9;
}
.paper {
  position: relative;
  width: 66%;
  height: 140px;
  padding: 24px 20px;
  background: #fffcf3;
  box-shadow: 0 12px 25px #4033210a;
  transform: rotate(8deg) translateY(10px);
}
.paper span {
  font:
    23px Georgia,
    serif;
  color: #8b7e5e;
  white-space: nowrap;
}
.paper i {
  display: block;
  height: 2px;
  background: #d8cfba;
  margin-top: 12px;
}
.paper i:last-child {
  width: 70%;
}
.app-artwork small {
  position: absolute;
  right: 5%;
  top: 22px;
  transform: rotate(-10deg);
  padding: 9px 15px;
  border-radius: 4px;
  background: #ecd599;
  color: #97814b;
  font-size: 10px;
}
.notes,
.pdf {
  background: #f2eee3;
}
.design {
  background: #eeebf4;
}
.design-board {
  display: flex;
  gap: 8px;
  width: 76%;
  height: 135px;
  padding: 12px;
  border-radius: 8px;
  background: white;
  transform: rotate(-5deg) translateY(15px);
  box-shadow: 0 14px 20px #29213c09;
}
.design-board i {
  width: 50%;
  border-radius: 5px;
  background: #b4c6bd;
}
.design-board span {
  display: grid;
  place-items: center;
  flex: 1;
  background: #e7cfbc;
  border-radius: 5px;
  font:
    40px Georgia,
    serif;
  color: #a97c62;
}
.swatches {
  display: flex;
  position: absolute;
  bottom: 23px;
  right: 7%;
  padding: 8px;
  gap: 5px;
  border-radius: 6px;
  background: white;
  transform: rotate(6deg);
}
.swatches i {
  width: 17px;
  height: 17px;
  background: #b1a2c8;
  border-radius: 50%;
}
.swatches i:nth-child(2) {
  background: #d6b29a;
}
.swatches i:nth-child(3) {
  background: #a8bfb3;
}
.browser {
  background: #e6eeee;
}
.browser-window {
  width: 80%;
  border-radius: 8px;
  overflow: hidden;
  transform: rotate(5deg) translateY(18px);
}
.browser-window header {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 22px;
  padding-left: 10px;
  background: #fbfdfd;
  color: #c8d2d4;
  font-size: 10px;
}
.browser-window header i {
  height: 6px;
  width: 65%;
  border-radius: 3px;
  background: #e8eeee;
}
.browser-window > div {
  height: 120px;
  background: linear-gradient(165deg, #d5e5e5 50%, #acc8be 50%);
  position: relative;
}
.browser-window span {
  position: absolute;
  width: 40px;
  height: 40px;
  background: #f5e9c9;
  border-radius: 50%;
  left: 25%;
  top: 14px;
}
.browser-window > div i {
  position: absolute;
  right: 0;
  bottom: -35px;
  background: #6c8f80;
  width: 65%;
  height: 110px;
  border-radius: 50%;
}
.workspace-window {
  display: flex;
  width: 78%;
  height: 135px;
  border: 9px solid white;
  border-radius: 8px;
  background: white;
  transform: rotate(-6deg) translateY(15px);
  box-shadow: 0 12px 30px #34493908;
}
.workspace-window .art-side {
  width: 24%;
  padding: 7px;
  background: #f2f3f4;
}
.workspace-window i {
  display: block;
  background: #d5dadd;
  height: 4px;
  margin: 9px 0;
  border-radius: 4px;
}
.workspace-window .art-main {
  flex: 1;
  padding: 0 12px;
}
.workspace-window .art-main i {
  margin: 15px 0;
  background: #e7e9ec;
}
.workspace-window .art-main i:first-child {
  background: #acb9ab;
  width: 60%;
}
.workspace .app-artwork small,
.sync .app-artwork small {
  top: auto;
  bottom: 25px;
  background: white;
  color: #7d9184;
  transform: rotate(4deg);
}
</style>
