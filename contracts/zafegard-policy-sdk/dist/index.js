import { Buffer } from "buffer";
import { Client as ContractClient, Spec as ContractSpec, } from '@stellar/stellar-sdk/minimal/contract';
if (typeof window !== 'undefined') {
    //@ts-ignore Buffer exists
    window.Buffer = window.Buffer || Buffer;
}
export const networks = {
    testnet: {
        networkPassphrase: "Test SDF Network ; September 2015",
        contractId: "NIL",
    }
};
export const Errors = {
    1: { message: "AlreadyInitialized" },
    2: { message: "NotInitialized" },
    3: { message: "NotFound" },
    4: { message: "NotAllowed" },
    5: { message: "TooSoon" },
    6: { message: "TooMuch" }
};
export class Client extends ContractClient {
    options;
    constructor(options) {
        super(new ContractSpec(["AAAAAgAAAAAAAAAAAAAACVNpZ25lcktleQAAAAAAAAMAAAABAAAAAAAAAAZQb2xpY3kAAAAAAAEAAAATAAAAAQAAAAAAAAAHRWQyNTUxOQAAAAABAAAD7gAAACAAAAABAAAAAAAAAAlTZWNwMjU2cjEAAAAAAAABAAAADg==",
            "AAAAAgAAAAAAAAAAAAAAClN0b3JhZ2VLZXkAAAAAAAIAAAAAAAAAAAAAAAVBZG1pbgAAAAAAAAEAAAAAAAAACFByZXZpb3VzAAAAAQAAA+4AAAAg",
            "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABgAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAAgAAAAAAAAAITm90Rm91bmQAAAADAAAAAAAAAApOb3RBbGxvd2VkAAAAAAAEAAAAAAAAAAdUb29Tb29uAAAAAAUAAAAAAAAAB1Rvb011Y2gAAAAABg==",
            "AAAAAAAAAAAAAAAEaW5pdAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAA=",
            "AAAAAAAAAAAAAAAKYWRkX3dhbGxldAAAAAAABAAAAAAAAAAEdXNlcgAAA+4AAAAgAAAAAAAAAANzYWMAAAAAEwAAAAAAAAAIaW50ZXJ2YWwAAAAEAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAA",
            "AAAAAAAAAAAAAAANcmVtb3ZlX3dhbGxldAAAAAAAAAEAAAAAAAAABHVzZXIAAAPuAAAAIAAAAAA=",
            "AAAAAAAAAAAAAAANdXBkYXRlX3dhbGxldAAAAAAAAAMAAAAAAAAABHVzZXIAAAPuAAAAIAAAAAAAAAAIaW50ZXJ2YWwAAAPoAAAABAAAAAAAAAAGYW1vdW50AAAAAAPoAAAACwAAAAA=",
            "AAAAAAAAAAAAAAAIcG9saWN5X18AAAADAAAAAAAAAAdfc291cmNlAAAAABMAAAAAAAAABnNpZ25lcgAAAAAH0AAAAAlTaWduZXJLZXkAAAAAAAAAAAAACGNvbnRleHRzAAAD6gAAB9AAAAAHQ29udGV4dAAAAAAA"]), options);
        this.options = options;
    }
    fromJSON = {
        init: (this.txFromJSON),
        add_wallet: (this.txFromJSON),
        remove_wallet: (this.txFromJSON),
        update_wallet: (this.txFromJSON),
    };
}
