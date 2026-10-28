<script setup lang="ts">
import { onMounted, ref } from "vue";
import Modal from "./Modal.vue";
import { store } from "../state/store";

const ok = ref<HTMLButtonElement>();
onMounted(() => ok.value?.focus());

function answer(yes: boolean) {
  store.confirm?.resolve(yes);
  store.confirm = null;
}
</script>

<template>
  <Modal v-if="store.confirm" :width="440" @close="answer(false)">
    <div class="confirm">
      <p>{{ store.confirm.text }}</p>
      <footer>
        <button class="btn btn-ghost" @click="answer(false)">Annuler</button>
        <button ref="ok" class="btn btn-danger" @click="answer(true)">{{ store.confirm.action }}</button>
      </footer>
    </div>
  </Modal>
</template>

<style scoped>
.confirm {
  padding: 20px;
}
p {
  color: var(--text);
  line-height: 1.55;
}
footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 18px;
}
</style>
