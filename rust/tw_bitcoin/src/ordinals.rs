use crate::{Error, Recipient, Result, TaprootProgram, TaprootScript};
use bitcoin::script::{PushBytesBuf, ScriptBuf};
use bitcoin::secp256k1::XOnlyPublicKey;
use bitcoin::taproot::{TaprootBuilder, TaprootSpendInfo};
use bitcoin::{PublicKey, Script};

#[derive(Debug, Clone)]
pub struct OrdinalsInscription {
    envelope: TaprootProgram,
    recipient: Recipient<TaprootScript>,
}

impl OrdinalsInscription {
    /// Creates a new Ordinals Inscription ("commit stage").
    pub fn new(
        mime: &[u8],
        data: &[u8],
        recipient: Recipient<PublicKey>,
    ) -> Result<OrdinalsInscription> {
        // Create the envelope, containing the inscription content.
        let envelope = create_envelope(mime, data, recipient.public_key())?;

        // Compute the merkle root of the inscription.
        let merkle_root = envelope
            .spend_info
            .merkle_root()
            .expect("Ordinals envelope not constructed correctly");

        Ok(OrdinalsInscription {
            envelope,
            recipient: Recipient::<TaprootScript>::from_pubkey_recipient(recipient, merkle_root),
        })
    }
    pub fn taproot_program(&self) -> &Script {
        self.envelope.script.as_script()
    }
    pub fn spend_info(&self) -> &TaprootSpendInfo {
        &self.envelope.spend_info
    }
    pub fn recipient(&self) -> &Recipient<TaprootScript> {
        &self.recipient
    }
}

/// Creates an [Ordinals Inscription](https://docs.ordinals.com/inscriptions.html).
/// This function is used for two purposes:
///
/// 1. It creates the spending condition for the given `internal_key`. This
///    associates the public key of the recipient with the Merkle root of the
///    Inscription on-chain, but it does not actually reveal the script to
///    anyone ("commit stage").
/// 2. The same function can then be used by the spender/claimer to actually
///    transfer the Inscripion by sending a transaction with the Inscription
///    script in the Witness ("reveal stage").
///
/// Do note that the `internal_key` can be different for each stage, but it
/// could also be the same entity. Stage one, the `internal_key` is the
/// recipient. Stage two, the `internal_key` is the claimer of the transaction
/// (where the Inscription script is available in the Witness).
fn create_envelope(mime: &[u8], data: &[u8], internal_key: PublicKey) -> Result<TaprootProgram> {
    use bitcoin::opcodes::all::*;
    use bitcoin::opcodes::*;

    // Create MIME buffer.
    let mut mime_buf = PushBytesBuf::new();
    mime_buf.extend_from_slice(mime).map_err(|_| Error::Todo)?;

    // Create data buffer.
    let mut data_buf = PushBytesBuf::new();
    data_buf.extend_from_slice(data).map_err(|_| Error::Todo)?;

    // Create an Ordinals Inscription.
    let builder = ScriptBuf::builder()
        .push_opcode(OP_FALSE)
        .push_opcode(OP_IF)
        .push_slice(b"ord")
        // Separator.
        .push_opcode(OP_PUSHBYTES_1);

    // The function `script::Builder::push_slice()` has a default behavior where
    // it only prefixes the length of the pushed bytes with an indicator when
    // the number of pushed bytes exceeds 75. Specifically, when the number of
    // bytes is 75 or less, the data pushed to the script has the format:
    // <DATA.len><DATA>
    //
    // But when the number of bytes exceeds 75, the function prefixes the data
    // with an opcode OP_PUSHDATA[1|2|4], creating a script in this format:
    // <OP_PUSHDATA[1|2|4]><DATA.len><DATA>
    //
    // However, when dealing with the MIME type of an Ordinal Inscription, the
    // requirements differ. The OP_PUSHDATA prefix is always needed, regardless
    // of whether the number of bytes pushed to the script is below 76.
    let builder = if data.len() < 76 {
        builder.push_opcode(OP_PUSHBYTES_1)
    } else {
        builder
    };

    let script = builder
        // MIME type identifying the data
        .push_slice(mime_buf.as_push_bytes())
        // Separator.
        .push_opcode(OP_PUSHBYTES_0)
        // The data itself.
        .push_slice(data_buf)
        .push_opcode(OP_ENDIF)
        .into_script();

    // Generate the necessary spending information. As mentioned in the
    // documentation of this function at the top, this serves two purposes;
    // setting the spending condition and actually claiming the spending
    // condition.
    let spend_info = TaprootBuilder::new()
        .add_leaf(0, script.clone())
        .expect("Ordinals Inscription spending info must always build")
        .finalize(
            &secp256k1::Secp256k1::new(),
            XOnlyPublicKey::from(internal_key.inner),
        )
        .expect("Ordinals Inscription spending info must always build");

    Ok(TaprootProgram { script, spend_info })
}
