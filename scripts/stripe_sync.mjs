#!/usr/bin/env node
/**
 * scripts/stripe_sync.mjs
 * Autonomous Stripe Catalog Sync & Restricted Key Capability Prober
 *
 * Usage:
 *   node scripts/stripe_sync.mjs --probe       # Probe API capabilities and catalog (dry-run)
 *   node scripts/stripe_sync.mjs --sync        # Idempotently sync missing products & prices
 *   node scripts/stripe_sync.mjs --update-env  # Sync and write price IDs to workspace-prototype/.env
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");
const envPath = path.resolve(projectRoot, "workspace-prototype/.env");

function loadEnvFile(filePath) {
  if (!fs.existsSync(filePath)) return {};
  const content = fs.readFileSync(filePath, "utf8");
  const env = {};
  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const match = trimmed.match(/^([A-Za-z0-9_]+)\s*=\s*(.*)$/);
    if (match) {
      env[match[1]] = match[2].trim().replace(/^["']|["']$/g, "");
    }
  }
  return env;
}

const env = loadEnvFile(envPath);
const apiKey = env.STRIPE_SECRET_KEY || env.STRIPE_API_KEY || process.env.STRIPE_SECRET_KEY || process.env.STRIPE_API_KEY;

if (!apiKey || apiKey.includes("your_restricted_key_here")) {
  console.error("❌ Error: Valid Stripe API key not found in workspace-prototype/.env");
  process.exit(1);
}

const STRIPE_API_BASE = "https://api.stripe.com/v1";

function buildSearchParams(params) {
  const usp = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null) continue;
    if (typeof value === "object" && !Array.isArray(value)) {
      for (const [subKey, subVal] of Object.entries(value)) {
        if (subVal !== undefined && subVal !== null) {
          usp.append(`${key}[${subKey}]`, String(subVal));
        }
      }
    } else {
      usp.append(key, String(value));
    }
  }
  return usp.toString();
}

async function stripeRequest(endpoint, method = "GET", bodyParams = null) {
  const options = {
    method,
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/x-www-form-urlencoded",
    },
  };
  if (bodyParams) {
    options.body = buildSearchParams(bodyParams);
  }
  const response = await fetch(`${STRIPE_API_BASE}${endpoint}`, options);
  const data = await response.json();
  return { status: response.status, ok: response.ok, data };
}

const CATALOG_DEFINITIONS = [
  {
    tier: "community",
    name: "Post Card Spatial OS — Community Edition",
    description: "Open-core bare-metal microkernel and spatial hypervisor with 2 vCPU allocation and standard 3D carousel.",
    tax_code: "txcd_10000000",
    metadata: {
      tier: "community",
      product_family: "post-card-spatial-os",
      max_vcpus: "2",
      max_concurrent_guests: "3",
      shadow_maps: "false",
      snapshots_enabled: "false",
      swarm_fork_enabled: "false",
      paper_plane_courier: "local_only",
      vfio_enabled: "false",
      airgap_vlan_enabled: "false",
      dmar_virtualization: "false",
      audit_logging: "false",
    },
    prices: [],
  },
  {
    tier: "pro",
    name: "Post Card Spatial OS — Pro Gamer Suite",
    description: "High-performance spatial hypervisor suite with 4 vCPU allocation, soft shadow maps, time-travel savepoints, and network courier.",
    tax_code: "txcd_10000000",
    metadata: {
      tier: "pro",
      product_family: "post-card-spatial-os",
      max_vcpus: "4",
      max_concurrent_guests: "8",
      shadow_maps: "true",
      snapshots_enabled: "true",
      swarm_fork_enabled: "true",
      paper_plane_courier: "network_mesh",
      vfio_enabled: "false",
      airgap_vlan_enabled: "false",
      dmar_virtualization: "false",
      audit_logging: "false",
    },
    prices: [
      {
        key: "STRIPE_PRICE_PRO_MONTHLY",
        unit_amount: 1900,
        currency: "usd",
        recurring: { interval: "month", interval_count: 1 },
        metadata: { tier: "pro", billing_cycle: "monthly" },
      },
      {
        key: "STRIPE_PRICE_PRO_ANNUAL",
        unit_amount: 19000,
        currency: "usd",
        recurring: { interval: "year", interval_count: 1 },
        metadata: { tier: "pro", billing_cycle: "annual", discount: "17_percent" },
      },
    ],
  },
  {
    tier: "enterprise",
    name: "Post Card Spatial OS — Enterprise SecOps & Vault Suite",
    description: "Zero-trust microkernel security vault with BitVisor VT-d DMAR virtualization, NVIDIA OpenShell VFIO hardware passthrough, and Kali Cyber Range.",
    tax_code: "txcd_10000000",
    metadata: {
      tier: "enterprise",
      product_family: "post-card-spatial-os",
      max_vcpus: "16",
      max_concurrent_guests: "16",
      shadow_maps: "true",
      snapshots_enabled: "true",
      swarm_fork_enabled: "true",
      paper_plane_courier: "network_mesh",
      vfio_enabled: "true",
      airgap_vlan_enabled: "true",
      dmar_virtualization: "true",
      audit_logging: "true",
      seat_tier: "per_seat",
    },
    prices: [
      {
        key: "STRIPE_PRICE_ENTERPRISE_MONTHLY",
        unit_amount: 19900,
        currency: "usd",
        billing_scheme: "per_unit",
        recurring: { interval: "month", interval_count: 1, usage_type: "licensed" },
        metadata: { tier: "enterprise", billing_cycle: "monthly", seat_based: "true" },
      },
      {
        key: "STRIPE_PRICE_ENTERPRISE_ANNUAL",
        unit_amount: 199000,
        currency: "usd",
        billing_scheme: "per_unit",
        recurring: { interval: "year", interval_count: 1, usage_type: "licensed" },
        metadata: { tier: "enterprise", billing_cycle: "annual", discount: "17_percent", seat_based: "true" },
      },
    ],
  },
];

async function run() {
  const args = process.argv.slice(2);
  const isProbeOnly = args.includes("--probe");
  const isSync = args.includes("--sync") || args.includes("--update-env");
  const isUpdateEnv = args.includes("--update-env");

  console.log("==========================================================");
  console.log("⚡ Post Card Spatial OS · Stripe Monetization Sync & Probe");
  console.log("==========================================================");
  console.log(`Key Prefix: ${apiKey.slice(0, 8)}... (Length: ${apiKey.length})`);

  // Probe capabilities
  console.log("\n[1/3] Probing Restricted Key Capabilities...");
  const probes = [
    { ep: "/products?limit=1", method: "GET", name: "Products Read" },
    { ep: "/prices?limit=1", method: "GET", name: "Prices Read" },
    { ep: "/payment_links?limit=1", method: "GET", name: "Payment Links Read" },
    { ep: "/checkout/sessions?limit=1", method: "GET", name: "Checkout Sessions Read" },
  ];
  for (const p of probes) {
    const res = await stripeRequest(p.ep, p.method);
    console.log(`  ✓ ${p.name.padEnd(25)}: HTTP ${res.status} (${res.ok ? "PERMITTED" : "DENIED"})`);
  }

  // Fetch existing inventory
  console.log("\n[2/3] Fetching Current Account Inventory...");
  const existingProductsRes = await stripeRequest("/products?limit=100");
  const existingProducts = existingProductsRes.ok ? existingProductsRes.data.data : [];
  console.log(`  Found ${existingProducts.length} existing products in Stripe.`);

  const existingPricesRes = await stripeRequest("/prices?limit=100&active=true");
  const existingPrices = existingPricesRes.ok ? existingPricesRes.data.data : [];
  console.log(`  Found ${existingPrices.length} active prices in Stripe.`);

  if (isProbeOnly && !isSync) {
    console.log("\nProbe complete. Pass --sync or --update-env to provision missing catalog entries.");
    return;
  }

  // Provisioning
  console.log("\n[3/3] Synchronizing Catalog...");
  const generatedPriceIds = {};

  for (const cat of CATALOG_DEFINITIONS) {
    let product = existingProducts.find(
      (p) => p.name === cat.name || p.metadata?.tier === cat.tier
    );
    if (!product) {
      console.log(`  ➕ Creating Product: "${cat.name}"...`);
      const createRes = await stripeRequest("/products", "POST", {
        name: cat.name,
        description: cat.description,
        metadata: cat.metadata,
        tax_code: cat.tax_code,
      });
      if (!createRes.ok) {
        console.error(`  ❌ Failed to create product ${cat.name}:`, createRes.data.error?.message);
        continue;
      }
      product = createRes.data;
      console.log(`     Created Product ID: ${product.id}`);
    } else {
      console.log(`  ✓ Product Exists: "${cat.name}" (${product.id})`);
      if (!product.tax_code && cat.tax_code) {
        console.log(`     Updating product tax code to ${cat.tax_code}...`);
        await stripeRequest(`/products/${product.id}`, "POST", { tax_code: cat.tax_code });
      }
    }

    // Sync prices
    for (const pr of cat.prices) {
      const price = existingPrices.find(
        (p) =>
          p.product === product.id &&
          p.unit_amount === pr.unit_amount &&
          p.recurring?.interval === pr.recurring.interval
      );
      if (!price) {
        console.log(`  ➕ Creating Price for ${cat.tier.toUpperCase()} (${pr.recurring.interval} @ $${pr.unit_amount / 100})...`);
        const pricePayload = {
          product: product.id,
          unit_amount: pr.unit_amount,
          currency: pr.currency,
          recurring: pr.recurring,
          metadata: pr.metadata,
        };
        if (pr.billing_scheme) pricePayload.billing_scheme = pr.billing_scheme;

        const createPriceRes = await stripeRequest("/prices", "POST", pricePayload);
        if (!createPriceRes.ok) {
          console.error(`  ❌ Failed to create price:`, createPriceRes.data.error?.message);
          continue;
        }
        console.log(`     Created Price ID: ${createPriceRes.data.id}`);
        generatedPriceIds[pr.key] = createPriceRes.data.id;
      } else {
        console.log(`  ✓ Price Exists: ${pr.key} -> ${price.id}`);
        generatedPriceIds[pr.key] = price.id;
      }
    }
  }

  // Update .env if requested
  if (isUpdateEnv && Object.keys(generatedPriceIds).length > 0) {
    console.log(`\nWriting resolved price IDs to ${envPath}...`);
    let currentContent = fs.existsSync(envPath) ? fs.readFileSync(envPath, "utf8") : "";
    for (const [key, id] of Object.entries(generatedPriceIds)) {
      const regex = new RegExp(`^${key}=.*$`, "m");
      if (regex.test(currentContent)) {
        currentContent = currentContent.replace(regex, `${key}=${id}`);
      } else {
        currentContent += `\n${key}=${id}`;
      }
    }
    // Also set default aliases
    if (generatedPriceIds.STRIPE_PRICE_PRO_MONTHLY) {
      if (/^STRIPE_PRICE_PRO=.*$/m.test(currentContent)) {
        currentContent = currentContent.replace(/^STRIPE_PRICE_PRO=.*$/m, `STRIPE_PRICE_PRO=${generatedPriceIds.STRIPE_PRICE_PRO_MONTHLY}`);
      } else {
        currentContent += `\nSTRIPE_PRICE_PRO=${generatedPriceIds.STRIPE_PRICE_PRO_MONTHLY}`;
      }
    }
    if (generatedPriceIds.STRIPE_PRICE_ENTERPRISE_MONTHLY) {
      if (/^STRIPE_PRICE_ENTERPRISE=.*$/m.test(currentContent)) {
        currentContent = currentContent.replace(/^STRIPE_PRICE_ENTERPRISE=.*$/m, `STRIPE_PRICE_ENTERPRISE=${generatedPriceIds.STRIPE_PRICE_ENTERPRISE_MONTHLY}`);
      } else {
        currentContent += `\nSTRIPE_PRICE_ENTERPRISE=${generatedPriceIds.STRIPE_PRICE_ENTERPRISE_MONTHLY}`;
      }
    }
    fs.writeFileSync(envPath, currentContent.trim() + "\n", "utf8");
    console.log("  ✓ Updated .env successfully.");
  }

  console.log("\n✨ Stripe Catalog Synchronization Completed Successfully.");
}

run().catch((err) => {
  console.error("Fatal error:", err.message);
  process.exit(1);
});
