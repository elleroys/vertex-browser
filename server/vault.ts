import crypto from 'node:crypto';
import { VortexDatabase } from './db.ts';

const VERIFIER_TEXT = 'VORTEX_VAULT_VERIFIED_V1';

export interface PasswordCredential {
  id: string;
  origin: string;
  title: string;
  username: string;
  password: string;
  notes: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface VaultStatus {
  isConfigured: boolean;
  isUnlocked: boolean;
  entryCount: number;
}

export class ServerPasswordVault {
  private masterKey: Buffer | null = null;
  private db: VortexDatabase;

  constructor(db: VortexDatabase) {
    this.db = db;
  }

  private deriveKey(masterPassword: string, salt: Buffer): Buffer {
    return crypto.scryptSync(masterPassword, salt, 32, { N: 16384, r: 8, p: 1 });
  }

  private encrypt(key: Buffer, plaintext: string): string {
    const iv = crypto.randomBytes(12);
    const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);
    const enc = Buffer.concat([cipher.update(plaintext, 'utf8'), cipher.final()]);
    const tag = cipher.getAuthTag();
    return Buffer.concat([iv, tag, enc]).toString('base64');
  }

  private decrypt(key: Buffer, encoded: string): string {
    const raw = Buffer.from(encoded, 'base64');
    if (raw.length < 28) throw new Error('Пошкоджений шифротекст');
    const iv = raw.subarray(0, 12);
    const tag = raw.subarray(12, 28);
    const ciphertext = raw.subarray(28);
    const decipher = crypto.createDecipheriv('aes-256-gcm', key, iv);
    decipher.setAuthTag(tag);
    return Buffer.concat([decipher.update(ciphertext), decipher.final()]).toString('utf8');
  }

  public normalizeOrigin(raw: string): string {
    try {
      const u = new URL(raw.trim().startsWith('http') ? raw.trim() : `https://${raw.trim()}`);
      return u.hostname.toLowerCase().replace(/^www\./, '');
    } catch {
      return raw.trim().toLowerCase().replace(/^www\./, '');
    }
  }

  public status(): VaultStatus {
    const meta = this.db.getVaultMeta();
    const entries = this.db.listPasswordRows();
    return {
      isConfigured: meta !== null,
      isUnlocked: this.masterKey !== null,
      entryCount: entries.length,
    };
  }

  public setup(masterPassword: string): void {
    if (masterPassword.length < 8) {
      throw new Error('Майстер-пароль має містити щонайменше 8 символів');
    }
    const salt = crypto.randomBytes(16);
    const key = this.deriveKey(masterPassword, salt);
    const verifierEnc = this.encrypt(key, VERIFIER_TEXT);
    this.db.saveVaultMeta(salt.toString('base64'), verifierEnc, 'scrypt:N=16384,r=8,p=1;aes-256-gcm');
    if (this.masterKey) this.masterKey.fill(0);
    this.masterKey = key;
  }

  public unlock(masterPassword: string): void {
    const meta = this.db.getVaultMeta();
    if (!meta) throw new Error('Сховище ще не налаштоване');
    const salt = Buffer.from(meta.saltB64, 'base64');
    const key = this.deriveKey(masterPassword, salt);
    try {
      const check = this.decrypt(key, meta.verifierEnc);
      if (check !== VERIFIER_TEXT) throw new Error('Невірний майстер-пароль');
    } catch {
      key.fill(0);
      throw new Error('Невірний майстер-пароль');
    }
    if (this.masterKey) this.masterKey.fill(0);
    this.masterKey = key;
  }

  public lock(): void {
    if (this.masterKey) {
      this.masterKey.fill(0);
      this.masterKey = null;
    }
  }

  private requireKey(): Buffer {
    if (!this.masterKey) throw new Error('Сховище паролів заблоковане');
    return this.masterKey;
  }

  public list(search = ''): PasswordCredential[] {
    const key = this.requireKey();
    const needle = search.trim().toLowerCase();
    const rows = this.db.listPasswordRows();
    const out: PasswordCredential[] = [];

    for (const r of rows) {
      const username = this.decrypt(key, r.usernameEnc);
      const password = this.decrypt(key, r.passwordEnc);
      const notes = r.notesEnc ? this.decrypt(key, r.notesEnc) : null;
      if (
        needle &&
        !r.origin.toLowerCase().includes(needle) &&
        !r.title.toLowerCase().includes(needle) &&
        !username.toLowerCase().includes(needle)
      ) {
        continue;
      }
      out.push({
        id: r.id,
        origin: r.origin,
        title: r.title,
        username,
        password,
        notes,
        createdAt: r.createdAt,
        updatedAt: r.updatedAt,
      });
    }
    return out;
  }

  public findForUrl(pageUrl: string): PasswordCredential[] {
    if (!this.masterKey) return [];
    const target = this.normalizeOrigin(pageUrl);
    if (!target) return [];
    return this.list('').filter((c) => c.origin === target || target.endsWith(`.${c.origin}`));
  }

  public save(input: {
    id?: string;
    origin: string;
    title: string;
    username: string;
    password: string;
    notes?: string | null;
  }): PasswordCredential {
    const key = this.requireKey();
    const now = Date.now();
    const id = input.id || crypto.randomUUID();
    const origin = this.normalizeOrigin(input.origin);
    const title = input.title.trim() || origin;

    this.db.upsertPasswordRow({
      id,
      origin,
      title,
      usernameEnc: this.encrypt(key, input.username),
      passwordEnc: this.encrypt(key, input.password),
      notesEnc: input.notes ? this.encrypt(key, input.notes) : null,
      createdAt: now,
      updatedAt: now,
    });

    return {
      id,
      origin,
      title,
      username: input.username,
      password: input.password,
      notes: input.notes || null,
      createdAt: now,
      updatedAt: now,
    };
  }

  public delete(id: string): void {
    this.requireKey();
    this.db.deletePasswordRow(id);
  }

  public importGoogleCsv(csvContent: string): number {
    this.requireKey();
    const lines = csvContent.split(/\r?\n/).filter((l) => l.trim().length > 0);
    if (lines.length < 2) return 0;

    const parseCsvLine = (line: string): string[] => {
      const fields: string[] = [];
      let current = '';
      let inQuotes = false;
      for (let i = 0; i < line.length; i++) {
        const ch = line[i];
        if (ch === '"') {
          if (inQuotes && line[i + 1] === '"') {
            current += '"';
            i++;
          } else {
            inQuotes = !inQuotes;
          }
        } else if (ch === ',' && !inQuotes) {
          fields.push(current);
          current = '';
        } else {
          current += ch;
        }
      }
      fields.push(current);
      return fields;
    };

    const headers = parseCsvLine(lines[0]).map((h) => h.trim().toLowerCase());
    const idxName = headers.indexOf('name');
    const idxUrl = headers.indexOf('url');
    const idxUser = headers.indexOf('username');
    const idxPass = headers.indexOf('password');
    const idxNote = headers.indexOf('note');

    if (idxUrl === -1 || idxPass === -1) {
      throw new Error('Невірний формат CSV. Очікуються колонки: name,url,username,password,note');
    }

    let count = 0;
    for (let i = 1; i < lines.length; i++) {
      const cols = parseCsvLine(lines[i]);
      const url = (cols[idxUrl] || '').trim();
      const password = cols[idxPass] || '';
      if (!url || !password) continue;

      const origin = this.normalizeOrigin(url);
      const title = (idxName >= 0 ? cols[idxName] : '')?.trim() || origin;
      const username = (idxUser >= 0 ? cols[idxUser] : '') || '';
      const notes = idxNote >= 0 ? cols[idxNote] || null : null;

      this.save({ origin, title, username, password, notes });
      count++;
    }
    return count;
  }
}
