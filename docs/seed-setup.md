# Hosting knowledge bases on a Cognitum Seed

Your knowledge bases always live on your Mac and answer queries locally — that
never depends on a Seed. Pushing a KB to a Seed is an *optional* extra that lets
the Seed answer queries on its own.

## Why "your Seed is in sensor mode"

A Cognitum Seed stores vectors at a single fixed dimension. The **first** vector
written to a fresh Seed locks that dimension for the whole store.

- A Seed that is collecting **sensor data** is locked at a small dimension
  (commonly 8). This is the normal, healthy state for a sensing Seed.
- Your knowledge bases are **384- or 1024-dimensional** (BGE embeddings).

Because the dimensions differ, a sensing Seed cannot also hold a KB at the same
time — one store, one dimension. So `learn push` / `learn ask --on-seed` keep
your KBs on the Mac and answer locally. **Nothing is broken.**

## To host KBs on a Seed instead of sensor data

A Seed can host KBs *or* collect sensor data, not both at once. If you want this
Seed to serve KBs:

1. Decide the KB dimension you'll use (e.g. 384 for BGE-small, 1024 for
   BGE-large) and use it consistently across the KBs you push.
2. Point at a **fresh** Seed store, or one already locked at that KB dimension.
   - A freshly provisioned / factory-reset Seed reports `dimension: 8` only
     because nothing has been written yet. The first KB you push **locks** it at
     the embedder's dimension.
   - A Seed that already holds sensor data is locked at the sensor dimension.
     Changing it means clearing that sensor store first — a deliberate,
     destructive step you should only take if you no longer need that data.
3. Push a KB:

   ```bash
   learn push <topic> --seed <seed-ip>
   ```

4. Verify the locked dimension:

   ```bash
   curl -sk https://<seed-ip>:8443/api/v1/status | jq .dimension
   ```

## Auto-push after every ingest

```bash
learn config set seed.address <seed-ip>
learn config set seed.auto_push true
```

With auto-push on, every `learn ingest` tries to push to the Seed afterward. If
the Seed is in sensor mode, the ingest still **succeeds** — the KB is saved on
your Mac — and you'll see an informational note rather than an error.
