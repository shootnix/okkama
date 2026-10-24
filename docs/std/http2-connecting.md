# HTTP2 verified connection opening

Http2Connecting(route,tlsPolicy[,localSettings,limits,clock]) is an explicit H2 profile. Constructor performs no DNS, socket, handshake or wire I/O. A supplied route resolves and checks numeric endpoints once; TCP racing uses those exact observations. HTTPS derives DNS/IP verification identity and SNI from the URL rather than substituting the selected numeric peer. Only actual ALPN `h2` is accepted; absent ALPN or HTTP/1.1 rejects before the H2 magic. HTTP means explicitly supplied prior knowledge, with no implicit h2c upgrade.

async acquisition() returns Http2Acquisition(session,ExchangeDetails); async session() returns the session. Verified TLS/clear transport is unpublished until the exact client preface succeeds. Raw ready-but-unclaimed acquisitions and failure/cancellation paths use structured cleanup and explicit unpublished anchors. Session close cancels/drains its I/O before closing the TLS owner, preventing synthetic already_closed failures from physical close racing cancellation.

The selected-policy public HttpClient supports the H2-only profile alongside its existing default H1 profile. Connection/header/body-idle deadlines and checked-peer reuse are retained. H2 streaming bodies are lazy and one-shot. The current shared pool coalesces same-key opening and multiplexes request/body leases on one physical owner.

Evidence: opener/session TCP+HTTPS three files /33 assertions PASS; public-client H2 constructor/HTTPS/TCP reuse three files /32 PASS, and existing H1 client/redirect/cookie/auth compatibility eleven files /89 PASS before the separate interpreter lexical ownership correction. The correction has direct red/green evidence and its required full gate passed: 700 files /12465 assertions. Final root H2 gate after settlement refinement: ten files /83 assertions PASS. No native provider or TLS verification was weakened.

Normative basis: [RFC 9113 sections 3.2–3.4](https://www.rfc-editor.org/rfc/rfc9113.html#section-3.2), exact TLS h2 selection or explicit cleartext prior knowledge before preface.
