import { useEffect, useState, type ChangeEvent, type FormEvent } from 'react';

type AccountMode = 'create' | 'login';
type CreatedAccount = { publicId: string; photoPending: boolean };

function isRegistrationResponse(value: unknown): value is { public_id: string; display_name: string } {
  return typeof value === 'object'
    && value !== null
    && 'public_id' in value
    && typeof value.public_id === 'string'
    && 'display_name' in value
    && typeof value.display_name === 'string';
}

const MAX_AVATAR_BYTES = 5 * 1024 * 1024;
const ACCEPTED_AVATAR_TYPES = ['image/jpeg', 'image/png', 'image/webp'];

function BrandMark() {
  return (
    <span className="brand-mark" aria-hidden="true">
      <svg viewBox="0 0 42 42" fill="none">
        <path d="M9 10.5h24v5H14v5h15v5H14v6H9V10.5Z" fill="currentColor" />
        <path d="M27 26.5h6v5h-6z" fill="#C7F36B" />
      </svg>
    </span>
  );
}

function ConnectionDiagram() {
  return (
    <div className="connection-diagram" aria-hidden="true">
      <svg className="connection-lines" viewBox="0 0 440 280" fill="none">
        <path d="M78 173 173 92l94 94 95-69" />
        <path d="M78 173h189M173 92l189 25" />
      </svg>
      <span className="connection-node node-left"><span className="node-avatar avatar-c">C</span></span>
      <span className="connection-node node-top"><span className="node-avatar avatar-d">D</span></span>
      <span className="connection-node node-center"><BrandMark /></span>
      <span className="connection-node node-right"><span className="node-avatar avatar-s">S</span></span>
      <span className="diagram-label label-left">CAMILA · EN LÍNEA</span>
      <span className="diagram-label label-right">ID · 7K4P2D9Q</span>
      <span className="diagram-caption">Una solicitud aceptada.<br />Una conversación que empieza.</span>
    </div>
  );
}

export default function App() {
  const [mode, setMode] = useState<AccountMode>('create');
  const [avatar, setAvatar] = useState<File | null>(null);
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const [avatarError, setAvatarError] = useState('');
  const [notice, setNotice] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [createdAccount, setCreatedAccount] = useState<CreatedAccount | null>(null);
  const [copyNotice, setCopyNotice] = useState('');

  useEffect(() => {
    if (!avatar) {
      setAvatarUrl(null);
      return;
    }

    const url = URL.createObjectURL(avatar);
    setAvatarUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [avatar]);

  function chooseAvatar(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0] ?? null;
    setAvatarError('');
    setNotice('');

    if (!file) {
      setAvatar(null);
      return;
    }

    if (!ACCEPTED_AVATAR_TYPES.includes(file.type)) {
      setAvatarError('Usá una foto JPG, PNG o WebP.');
      event.currentTarget.value = '';
      return;
    }

    if (file.size > MAX_AVATAR_BYTES) {
      setAvatarError('La foto debe pesar 5 MB o menos.');
      event.currentTarget.value = '';
      return;
    }

    setAvatar(file);
  }

  async function submitAccount(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (mode === 'login') {
      setNotice('El inicio de sesión se habilitará en la siguiente etapa.');
      return;
    }

    const form = event.currentTarget;
    const formData = new FormData(form);
    const photoPending = avatar !== null;
    const payload = {
      display_name: String(formData.get('displayName') ?? '').trim(),
      email: String(formData.get('email') ?? '').trim(),
      password: String(formData.get('password') ?? ''),
    };

    setSubmitting(true);
    setNotice('Creando tu cuenta…');
    setAvatarError('');

    try {
      const response = await fetch('/api/v1/auth/register', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const result: unknown = await response.json().catch(() => null);

      if (!response.ok) {
        const message = typeof result === 'object'
          && result !== null
          && 'message' in result
          && typeof result.message === 'string'
          ? result.message
          : 'No se pudo crear la cuenta. Intentá de nuevo.';
        setNotice(message);
        return;
      }

      if (!isRegistrationResponse(result)) {
        setNotice('El servidor respondió de una forma inesperada. Intentá de nuevo.');
        return;
      }

      setCreatedAccount({ publicId: result.public_id, photoPending });
      setCopyNotice('');
      setNotice('');
      form.reset();
      setAvatar(null);
    } catch {
      setNotice('No se pudo conectar con el servidor. Revisá tu conexión e intentá de nuevo.');
    } finally {
      setSubmitting(false);
    }
  }

  function changeMode(nextMode: AccountMode) {
    setMode(nextMode);
    setNotice('');
    setAvatarError('');
    setCreatedAccount(null);
    setCopyNotice('');
  }

  async function copyPublicId() {
    if (!createdAccount) return;

    try {
      await navigator.clipboard.writeText(createdAccount.publicId);
      setCopyNotice('ID copiado.');
    } catch {
      setCopyNotice('No se pudo copiar automáticamente; copiá el ID manualmente.');
    }
  }

  return (
    <main className="page-shell">
      <header className="topbar">
        <a className="brand" href="/" aria-label="ENTRE, inicio">
          <BrandMark />
          <span>ENTRE</span>
          <span className="brand-divider" />
          <span className="brand-caption">CHAT PRIVADO</span>
        </a>
        <div className="topbar-meta">
          <span className="service-indicator"><i /> ESPACIO EN PREPARACIÓN</span>
          <a className="topbar-login" href="#account" onClick={() => changeMode('login')}>Ingresar</a>
        </div>
      </header>

      <section className="main-grid" aria-labelledby="page-title">
        <div className="intro-column">
          <div className="eyebrow"><span className="eyebrow-line" /> CONVERSACIONES ENTRE PERSONAS</div>
          <h1 id="page-title">Hablá con<br />quien <em>elegís.</em></h1>
          <p className="intro-copy">Encontrá a alguien por su ID, conectá con su permiso y pasen del mensaje a la voz.</p>

          <div className="feature-list" aria-label="Funciones de la aplicación">
            <div className="feature-row">
              <span className="feature-number">01</span>
              <span className="feature-name">Un ID para encontrarse</span>
              <span className="feature-detail">privado · compartible</span>
            </div>
            <div className="feature-row">
              <span className="feature-number">02</span>
              <span className="feature-name">Mensajes solo entre contactos</span>
              <span className="feature-detail">con aceptación</span>
            </div>
            <div className="feature-row">
              <span className="feature-number">03</span>
              <span className="feature-name">Llamadas de voz 1 a 1</span>
              <span className="feature-detail">en tiempo real</span>
            </div>
          </div>

          <ConnectionDiagram />
          <p className="intro-footnote">Tu correo se queda privado. Tu ID lo compartís vos.</p>
        </div>

        <section className="account-card" id="account" aria-label={mode === 'create' ? 'Crear cuenta' : 'Ingresar'}>
          <div className="card-topline">
            <span className="card-kicker">TU ESPACIO</span>
            <span className="card-step">{mode === 'create' ? '01 / 02' : '02 / 02'}</span>
          </div>
          <h2>{createdAccount ? 'Tu ID está listo.' : mode === 'create' ? 'Empezá por vos.' : 'Qué bueno verte.'}</h2>
          <p className="card-intro">
            {createdAccount
              ? 'Compartí este código para que otras personas puedan encontrarte.'
              : mode === 'create'
              ? 'Creá tu cuenta; después te damos un ID para compartir.'
              : 'Entrá para volver a tus conversaciones.'}
          </p>

          <div className="mode-switch" role="tablist" aria-label="Acceso">
            <button className={mode === 'create' ? 'mode-tab active' : 'mode-tab'} type="button" role="tab" aria-selected={mode === 'create'} onClick={() => changeMode('create')}>Crear cuenta</button>
            <button className={mode === 'login' ? 'mode-tab active' : 'mode-tab'} type="button" role="tab" aria-selected={mode === 'login'} onClick={() => changeMode('login')}>Ingresar</button>
          </div>

          {createdAccount ? (
            <div className="registration-success" role="status">
              <span className="success-label">CUENTA CREADA</span>
              <div className="public-id-row">
                <code>{createdAccount.publicId}</code>
                <button className="copy-id-button" type="button" onClick={copyPublicId}>Copiar ID</button>
              </div>
              <p className="registration-note">
                El inicio de sesión todavía no está conectado.
                {createdAccount.photoPending && ' La foto que elegiste no se guardó; conectaremos su almacenamiento después.'}
              </p>
              {copyNotice && <p className="form-notice" aria-live="polite">{copyNotice}</p>}
            </div>
          ) : (
          <form className="account-form" onSubmit={submitAccount} aria-busy={submitting}>
            {mode === 'create' && (
              <>
                <div className="avatar-field">
                  <label className="avatar-picker" htmlFor="avatar-file" aria-label="Elegir foto de perfil">
                    {avatarUrl ? <img src={avatarUrl} alt="Vista previa de tu foto de perfil" /> : <span className="avatar-placeholder">+</span>}
                    <span className="avatar-camera" aria-hidden="true">
                      <svg viewBox="0 0 16 16" fill="none"><path d="M2.5 5.5h2l1-1.5h4l1 1.5h2v6h-10v-6Z" stroke="currentColor" strokeWidth="1.2"/><circle cx="7.5" cy="8.5" r="2" stroke="currentColor" strokeWidth="1.2"/></svg>
                    </span>
                    <input id="avatar-file" type="file" accept="image/jpeg,image/png,image/webp" onChange={chooseAvatar} />
                  </label>
                  <div className="avatar-instructions">
                    <span className="field-label">FOTO DE PERFIL <span>OPCIONAL</span></span>
                    <label htmlFor="avatar-file" className="avatar-action">{avatar ? 'Cambiar foto' : 'Elegir una foto'}</label>
                    <span className="avatar-hint">JPG, PNG o WebP · guardado en la siguiente etapa</span>
                  </div>
                </div>
                {avatarError && <p className="field-message error" role="alert">{avatarError}</p>}
                <label className="field-group">
                  <span className="field-label">NOMBRE VISIBLE</span>
                  <input type="text" name="displayName" autoComplete="name" placeholder="¿Cómo te van a ver?" minLength={2} maxLength={40} required />
                </label>
              </>
            )}

            <label className="field-group">
              <span className="field-label">CORREO ELECTRÓNICO</span>
              <input type="email" name="email" autoComplete="email" placeholder="vos@correo.com" maxLength={254} required />
            </label>

            <label className="field-group">
              <span className="field-label">CONTRASEÑA</span>
              <input type="password" name="password" autoComplete={mode === 'create' ? 'new-password' : 'current-password'} placeholder="Mínimo 12 caracteres" minLength={12} maxLength={128} required />
            </label>

            <button className="submit-button" type="submit" disabled={submitting || mode === 'login'}>
              {submitting ? 'Creando cuenta…' : mode === 'create' ? 'Crear mi cuenta' : 'Ingresar'}
              <svg viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="M4 10h11M10 5l5 5-5 5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/></svg>
            </button>
            <p className="form-notice" aria-live="polite">{notice || (mode === 'create' ? 'Tu contraseña no se guarda en texto claro.' : 'El inicio de sesión se habilitará en la siguiente etapa.')}</p>
          </form>
          )}

          <div className="card-bottom">
            <span className="lock-icon" aria-hidden="true"><svg viewBox="0 0 16 16" fill="none"><rect x="3" y="7" width="10" height="7" rx="1.5" stroke="currentColor" strokeWidth="1.2"/><path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" stroke="currentColor" strokeWidth="1.2"/></svg></span>
            <span>El ingreso y el guardado de fotos se conectarán después.</span>
          </div>
        </section>
      </section>

      <footer className="page-footer">
        <span>ENTRE <span className="footer-muted">/ NOMBRE PROVISIONAL</span></span>
        <span>CHAT PRIVADO · VOZ 1 A 1</span>
        <a href="/health" target="_blank" rel="noreferrer">ESTADO DEL SERVIDOR</a>
      </footer>
    </main>
  );
}
