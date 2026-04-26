/**
 * crypto — AES-256-GCM envelope helpers for the wallet store.
 *
 * Authenticated encryption: any tampering with the ciphertext or
 * IV causes `aesGcmDecrypt` to throw. Each call uses a fresh random
 * IV so identical plaintexts encrypt to distinct ciphertexts.
 *
 * The DEK (32-byte data encryption key) is supplied by a `KeyStore`
 * implementation — either derived from a passphrase via scrypt, or
 * pulled from an OS keychain (Phase 2).
 */

import { createCipheriv, createDecipheriv, randomBytes } from "crypto";

const ALGO = "aes-256-gcm";
export const KEY_LEN = 32;
const IV_LEN = 12;
const TAG_LEN = 16;

export interface EncryptedEnvelope {
  /** Base64-encoded 12-byte IV. */
  iv_b64: string;
  /** Base64-encoded ciphertext with the 16-byte GCM auth tag appended. */
  ciphertext_b64: string;
}

export function aesGcmEncrypt(
  plaintext: Buffer,
  key: Buffer
): EncryptedEnvelope {
  if (key.length !== KEY_LEN) {
    throw new Error(
      `aesGcmEncrypt: key must be ${KEY_LEN} bytes, got ${key.length}`
    );
  }
  const iv = randomBytes(IV_LEN);
  const cipher = createCipheriv(ALGO, key, iv);
  const ct = Buffer.concat([cipher.update(plaintext), cipher.final()]);
  const tag = cipher.getAuthTag();
  return {
    iv_b64: iv.toString("base64"),
    ciphertext_b64: Buffer.concat([ct, tag]).toString("base64"),
  };
}

export function aesGcmDecrypt(
  envelope: EncryptedEnvelope,
  key: Buffer
): Buffer {
  if (key.length !== KEY_LEN) {
    throw new Error(
      `aesGcmDecrypt: key must be ${KEY_LEN} bytes, got ${key.length}`
    );
  }
  const iv = Buffer.from(envelope.iv_b64, "base64");
  if (iv.length !== IV_LEN) {
    throw new Error(
      `aesGcmDecrypt: IV must be ${IV_LEN} bytes, got ${iv.length}`
    );
  }
  const combined = Buffer.from(envelope.ciphertext_b64, "base64");
  if (combined.length < TAG_LEN) {
    throw new Error(
      `aesGcmDecrypt: ciphertext too short (${combined.length} bytes) to contain ${TAG_LEN}-byte tag`
    );
  }
  const tag = combined.subarray(combined.length - TAG_LEN);
  const ciphertext = combined.subarray(0, combined.length - TAG_LEN);
  const decipher = createDecipheriv(ALGO, key, iv);
  decipher.setAuthTag(tag);
  return Buffer.concat([decipher.update(ciphertext), decipher.final()]);
}
