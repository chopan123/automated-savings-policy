<script lang="ts">
    import { Client } from "zafegard-policy-sdk";
    import {
        PasskeyServer,
        PasskeyKit,
        PasskeyClient,
        SACClient,
    } from "passkey-kit";
    import { fundPubkey, fundSigner, fundKeypair } from "../lib/common";
    import { onMount } from "svelte";
    import {
        xdr,
        Keypair,
        scValToNative,
        Operation,
        TransactionBuilder,
        Address,
        SorobanRpc,
        StrKey,
        hash,
    } from "@stellar/stellar-sdk-legacy";

    const pk_server = new PasskeyServer({
        rpcUrl: import.meta.env.PUBLIC_RPC_URL,
        launchtubeUrl: import.meta.env.PUBLIC_LAUNCHTUBE_URL,
        launchtubeJwt: import.meta.env.PUBLIC_LAUNCHTUBE_JWT,
    });

    const pk_wallet = new PasskeyKit({
        rpcUrl: import.meta.env.PUBLIC_RPC_URL,
        networkPassphrase: import.meta.env.PUBLIC_PASSPHRASE,
        factoryContractId: import.meta.env.PUBLIC_FACTORY,
    });

    const sac = new SACClient({
        rpcUrl: import.meta.env.PUBLIC_RPC_URL,
        networkPassphrase: import.meta.env.PUBLIC_PASSPHRASE,
    });

    const native = sac.getSACClient(import.meta.env.PUBLIC_NATIVE);

    let balance_: bigint;
    let keyId_: string;
    let contractId_: string;

    let url: URL;
    let contract: Client;
    let zafeguardPolicy: string;
    let subwallets: Map<string, [string, number, number]> = new Map();
    let loading: Map<string, boolean> = new Map();

    onMount(async () => {
        url = new URL(location.href);

        const keyId = url.searchParams.get("keyId") || undefined;
        const contractId = url.searchParams.get("contractId") || undefined;
        const secret = url.searchParams.get("secret") || undefined;

        if (keyId) {
            setSubWallets();
            await connectWallet(keyId).then(async () => {
                await initWallet();
                await fundWallet();
            });
        } else if (contractId && secret) {
            contractId_ = contractId;

            setZafeGuardPolicy();

            const keypair = Keypair.fromSecret(secret);
            const pubkey = keypair.publicKey();

            const [interval_scval, amount_scval] = await pk_wallet.rpc
                .getContractData(
                    zafeguardPolicy,
                    xdr.ScVal.scvBytes(keypair.rawPublicKey()),
                )
                .then(
                    ({ val }) =>
                        val.contractData().val().vec() as [
                            xdr.ScVal,
                            xdr.ScVal,
                        ],
                );
            const interval = interval_scval.u32();
            const amount = Number(amount_scval.i128().lo().toBigInt());

            subwallets = new Map([[pubkey, [secret, interval, amount]]]);

            // will be missing keyId_ but that's fine, just won't be able to sign with a passkey
            pk_wallet.wallet = new PasskeyClient({
                contractId,
                rpcUrl: import.meta.env.PUBLIC_RPC_URL,
                networkPassphrase: import.meta.env.PUBLIC_PASSPHRASE,
            });

            await fundWallet();
        } else if (localStorage.hasOwnProperty("zg:subwallets")) {
            localStorage.removeItem("zg:subwallets");
        }
    });

    async function createWallet() {
        try {
            loading.set("createWallet", true);
            loading = loading;

            const { keyId_base64, contractId, built } =
                await pk_wallet.createWallet("Zafegard", "Zafegard Admin");

            keyId_ = keyId_base64;

            const res = await pk_server.send(built);

            console.log(res);

            url.searchParams.set("keyId", keyId_);
            history.pushState({}, "", url);

            contractId_ = contractId;

            await initWallet();
            await fundWallet();
        } finally {
            loading.set("createWallet", false);
            loading = loading;
        }
    }
    async function connectWallet(keyId: string) {
        const { keyId_base64, contractId } = await pk_wallet.connectWallet({
            keyId,
        });

        keyId_ = keyId_base64;

        if (!keyId) {
            url.searchParams.set("keyId", keyId_);
            history.pushState({}, "", url);
        }

        contractId_ = contractId;
    }
    async function initWallet() {
        try {
            const rpc = new SorobanRpc.Server(import.meta.env.PUBLIC_RPC_URL);
            const source = await rpc.getAccount(fundPubkey);
            const transaction_before = new TransactionBuilder(source, {
                fee: "0",
                networkPassphrase: import.meta.env.PUBLIC_PASSPHRASE,
            })
                .addOperation(
                    Operation.createCustomContract({
                        address: Address.fromString(fundPubkey),
                        wasmHash: Buffer.from(
                            import.meta.env.PUBLIC_ZAFEGARD_POLICY_WASM,
                            "hex",
                        ),
                        salt: Address.fromString(contractId_).toBuffer(),
                    }),
                )
                .setTimeout(300)
                .build();

            const sim = await rpc.simulateTransaction(transaction_before);

            if (!SorobanRpc.Api.isSimulationSuccess(sim))
                throw new Error("Simulation failed");

            const transaction_after = TransactionBuilder.cloneFrom(
                transaction_before,
                {
                    fee: (Number(sim.minResourceFee) + 10_000_000).toString(),
                    sorobanData: sim.transactionData.build(),
                },
            ).build();

            const op = transaction_after
                .operations[0] as Operation.InvokeHostFunction;

            op.auth![0] = sim.result!.auth[0];

            transaction_after.sign(await fundKeypair);

            const res1 = await rpc._sendTransaction(transaction_after);

            if (res1.status !== "PENDING")
                return alert("Transaction send failed");

            await new Promise((resolve) => setTimeout(resolve, 6000));

            const res2 = await rpc.getTransaction(res1.hash);

            if (res2.status !== "SUCCESS") return alert("Transaction failed");

            console.log(res2);

            zafeguardPolicy = Address.contract(
                res2.returnValue!.address().contractId(),
            ).toString();
        } catch {
            setZafeGuardPolicy();
        }

        contract = new Client({
            rpcUrl: import.meta.env.PUBLIC_RPC_URL,
            contractId: zafeguardPolicy,
            networkPassphrase: import.meta.env.PUBLIC_PASSPHRASE,
        });

        const instance = await pk_wallet.rpc.getContractData(
            zafeguardPolicy,
            xdr.ScVal.scvLedgerKeyContractInstance(),
        );
        const admin = instance.val
            .contractData()
            .val()
            .instance()
            .storage()
            ?.filter((item) => {
                if (scValToNative(item.key())?.[0] === "Admin") {
                    return true;
                }
            });

        if (admin?.length) return;

        const at = await contract.init({
            admin: contractId_,
        });

        const res = await pk_server.send(at.built!);

        console.log(res);
    }
    async function fundWallet() {
        await setBalance();

        if (balance_ >= 100_000_000) return;

        const { built, ...transfer } = await native.transfer({
            to: contractId_,
            from: fundPubkey,
            amount: BigInt(100_000_000),
        });

        await transfer.signAuthEntries({
            address: fundPubkey,
            signAuthEntry: fundSigner.signAuthEntry,
        });

        const res = await pk_server.send(built!);

        console.log(res);

        await setBalance();
    }

    async function addSubWallet() {
        try {
            loading.set("addSubWallet", true);
            loading = loading;

            const keypair = Keypair.random();
            const interval = 10;
            const amount = 100;

            const at = await contract.add_wallet({
                user: keypair.rawPublicKey(),
                sac: import.meta.env.PUBLIC_NATIVE,
                interval,
                amount: BigInt(amount),
            });

            await pk_wallet.sign(at, { keyId: keyId_ });

            const res = await pk_server.send(at);

            console.log(res);

            localStorage.setItem(
                "zg:subwallets",
                JSON.stringify({
                    ...JSON.parse(
                        localStorage.getItem("zg:subwallets") || "{}",
                    ),
                    [keypair.publicKey()]: [keypair.secret(), interval, amount],
                }),
            );

            setSubWallets();
        } finally {
            loading.set("addSubWallet", false);
            loading = loading;
        }
    }
    async function removeSubWallet(pubkey: string) {
        try {
            loading.set(`remove_${pubkey}`, true);
            loading = loading;

            const at = await contract.remove_wallet({
                user: Keypair.fromPublicKey(pubkey).rawPublicKey(),
            });

            await pk_wallet.sign(at, { keyId: keyId_ });

            const res = await pk_server.send(at);

            console.log(res);

            localStorage.setItem(
                "zg:subwallets",
                JSON.stringify(
                    Object.fromEntries(
                        [...subwallets].filter(([key]) => key !== pubkey),
                    ),
                ),
            );

            setSubWallets();
        } finally {
            loading.set(`remove_${pubkey}`, false);
            loading = loading;
        }
    }
    async function updateSubWallet(secret: string, pubkey: string) {
        try {
            loading.set(`update_${pubkey}`, true);
            loading = loading;

            const interval = prompt("Enter new interval", "10");

            if (!interval) return;

            const amount = prompt("Enter new amount", "100");

            if (!amount) return;

            const at = await contract.update_wallet({
                user: Keypair.fromPublicKey(pubkey).rawPublicKey(),
                interval: Number(interval),
                amount: BigInt(amount),
            });

            await pk_wallet.sign(at, { keyId: keyId_ });

            const res = await pk_server.send(at);

            console.log(res);

            localStorage.setItem(
                "zg:subwallets",
                JSON.stringify({
                    ...JSON.parse(
                        localStorage.getItem("zg:subwallets") || "{}",
                    ),
                    [pubkey]: [secret, interval, amount],
                }),
            );

            setSubWallets();
        } finally {
            loading.set(`update_${pubkey}`, false);
            loading = loading;
        }
    }
    async function spendSubWallet(
        secret: string,
        interval: number,
        amount: number,
    ) {
        const keypair = Keypair.fromSecret(secret);
        const pubkey = keypair.publicKey();

        try {
            loading.set(`spend_${pubkey}`, true);
            loading = loading;

            try {
                const previous = await pk_wallet.rpc
                    .getContractData(
                        zafeguardPolicy,
                        xdr.ScVal.scvVec([
                            xdr.ScVal.scvSymbol("Previous"),
                            xdr.ScVal.scvBytes(keypair.rawPublicKey()),
                        ]),
                    )
                    .then(({ val }) => val.contractData().val().u32());

                const { sequence: current } =
                    await pk_wallet.rpc.getLatestLedger();

                amount = amount * Math.floor((current - previous) / interval);
            } catch {}

            if (!amount) {
                return alert("Error: TooSoon");
            }

            const at = await native.transfer({
                from: contractId_,
                to: fundPubkey,
                amount: BigInt(amount),
            });

            await pk_wallet.sign(at, { keypair });

            try {
                const res = await pk_server.send(at);

                console.log(res);
            } catch {
                alert("Transaction failed");
            }

            await setBalance();
        } finally {
            loading.set(`spend_${pubkey}`, false);
            loading = loading;
        }
    }

    function setZafeGuardPolicy() {
        const contractPreimage = xdr.HashIdPreimage.envelopeTypeContractId(
            new xdr.HashIdPreimageContractId({
                networkId: hash(
                    Buffer.from(import.meta.env.PUBLIC_PASSPHRASE, "utf8"),
                ),
                contractIdPreimage:
                    xdr.ContractIdPreimage.contractIdPreimageFromAddress(
                        new xdr.ContractIdPreimageFromAddress({
                            address:
                                Address.fromString(fundPubkey).toScAddress(),
                            salt: Address.fromString(contractId_).toBuffer(),
                        }),
                    ),
            }),
        );

        zafeguardPolicy = Address.fromString(
            StrKey.encodeContract(hash(contractPreimage.toXDR())),
        ).toString();
    }
    async function setBalance() {
        balance_ = await native
            .balance({
                id: contractId_,
            })
            .then(({ result }) => result)
            .catch(() => BigInt(0));

        console.log(balance_);
    }
    function setSubWallets() {
        subwallets = new Map(
            Object.entries(
                JSON.parse(localStorage.getItem("zg:subwallets") || "{}"),
            ),
        );
    }
    function compactAddress(address: string) {
        return address.slice(0, 5) + "..." + address.slice(-5);
    }
    function signOut() {
        keyId_ = "";
        contractId_ = "";
        localStorage.removeItem("zg:subwallets");
        location.assign(location.origin);
    }
</script>

<h1 class="text-2xl font-bold">Zafegard</h1>

{#if contractId_}
    <p>
        {contractId_}
        <button class="bg-black text-white px-2 py-1 rounded" on:click={signOut}
            >Sign Out</button
        >
    </p>

    {#if balance_}
        <p>
            {parseFloat((Number(balance_) / 10_000_000).toFixed(7)).toString()} XLM
        </p>
    {/if}

    <table class="border mt-2">
        <thead class="[&>tr>th]:px-2 [&>tr>th]:py-1">
            <tr class="bg-black text-white text-left">
                <th>Address</th>
                <th>Secret</th>
                <th>Amount</th>
                <th colspan="3">Interval</th>
            </tr>
        </thead>
        <tbody class="[&>tr>td]:px-2 [&>tr>td]:py-1">
            {#each subwallets as [pubkey, [secret, interval, amount]]}
                <tr class="odd:bg-slate-100">
                    <td>{compactAddress(pubkey)}</td>
                    <td>{compactAddress(secret)}</td>
                    <td>
                        {parseInt(amount.toString()).toLocaleString()} (stroops)
                    </td>
                    <td>
                        {parseInt(interval.toString()).toLocaleString()} (ledgers)
                    </td>
                    <td>
                        {#if keyId_}
                            <button
                                class="bg-purple-500 text-white px-2 py-1 rounded"
                                on:click={() => updateSubWallet(secret, pubkey)}
                            >
                                {#if loading.get(`update_${pubkey}`)}
                                    ...
                                {:else}
                                    Update
                                {/if}
                            </button>
                        {/if}

                        <button
                            class="bg-blue-500 text-white px-2 py-1 rounded"
                            on:click={() =>
                                spendSubWallet(secret, interval, amount)}
                        >
                            {#if loading.get(`spend_${pubkey}`)}
                                ...
                            {:else}
                                Spend
                            {/if}
                        </button>
                        <a
                            class="bg-black text-white px-2 py-1 rounded inline-block"
                            href={location.origin +
                                `?contractId=${contractId_}&secret=${secret}`}
                            target="_blank"
                            rel="nofollow"
                        >
                            Share
                        </a>
                    </td>
                    {#if keyId_}
                        <td class="text-right">
                            <button
                                class="bg-red-500 text-white px-2 py-1 rounded"
                                on:click={() => removeSubWallet(pubkey)}
                            >
                                {#if loading.get(`remove_${pubkey}`)}
                                    ...
                                {:else}
                                    –
                                {/if}
                            </button>
                        </td>
                    {/if}
                </tr>
            {/each}

            {#if keyId_ && balance_}
                <tr class="bg-slate-300">
                    <td colspan="5">Add new sub wallet</td>
                    <td class="text-right">
                        <button
                            class="bg-green-500 text-white px-2 py-1 rounded"
                            on:click={addSubWallet}
                        >
                            {#if loading.get("addSubWallet")}
                                ...
                            {:else}
                                +
                            {/if}
                        </button>
                    </td>
                </tr>
            {/if}

            {#if !balance_}
                <tr class="bg-slate-300">
                    <td colspan="6">Setting things up...</td>
                </tr>
            {/if}
        </tbody>
    </table>
{:else}
    <button
        class="bg-black text-white px-2 py-1 rounded mt-2"
        on:click={createWallet}
    >
        {#if loading.get("createWallet")}
            ...
        {:else}
            Sign Up
        {/if}
    </button>
{/if}
