<script lang="ts">
  export let baudRate: number;
  export let connected: boolean;
  export let status: string;
  export let detail: string;
  export let onBaudRateChange: (value: number) => void;
  export let onConnect: () => void;
  export let onDisconnect: () => void;
</script>

<section class="connection" aria-label="Serial connection">
  <label for="baud-rate">Baud rate</label>
  <input id="baud-rate" type="number" min="1200" step="1200" value={baudRate} disabled={connected} oninput={(event) => onBaudRateChange(Number(event.currentTarget.value))} />
  {#if connected}
    <button class="button-secondary" onclick={onDisconnect}>Disconnect</button>
  {:else}
    <button onclick={onConnect}>Connect serial port</button>
  {/if}
  <p class:online={connected} class="connection-status"><b>{status}</b>{detail}</p>
</section>
