    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    // Nav state
    const nav = document.getElementById('nav');
    addEventListener('scroll', () => nav.classList.toggle('scrolled', scrollY > 24), { passive: true });

    // Reveal on scroll
    const io = new IntersectionObserver((es) => es.forEach(e => {
      if (e.isIntersecting) { e.target.classList.add('in'); io.unobserve(e.target); }
    }), { threshold: 0.12 });
    document.querySelectorAll('.reveal').forEach(el => io.observe(el));

    // Certificate ribbon (illustrative — not live data)
    const track = document.getElementById('track');
    const hex = () => Array.from({ length: 6 }, () => '0123456789abcdef'[Math.random() * 16 | 0]).join('');
    const cards = Array.from({ length: 9 }, (_, i) =>
      `<div class="blk"><div class="t">CERT · n${i ? '+' + i : ''}</div><div>root ${hex()}…</div><div>prev ${hex()}…</div>` +
      `<div class="seals"><span class="seal bls">BLS</span><span class="seal pq">MAYO2</span></div></div>`).join('');
    track.innerHTML = cards + cards;

    // Pace lanes: one lit cell per finalized block at the measured pace (illustrative, not live data)
    document.querySelectorAll('.lane').forEach(lane => {
      const cells = lane.querySelector('.cells'), count = lane.querySelector('.lane-n');
      const n = innerWidth < 640 ? 20 : 36, ms = +lane.dataset.ms;
      cells.style.gridTemplateColumns = `repeat(${n}, 1fr)`;
      cells.innerHTML = '<i class="cell"></i>'.repeat(n);
      const list = cells.children;
      if (reduce) {
        const lit = lane.classList.contains('fast') ? n : Math.round(n * 110 / ms);
        for (let k = 0; k < lit; k++) list[k].classList.add('on');
        count.textContent = (1000 / ms).toFixed(1) + ' blocks/s';
        return;
      }
      let k = 0, total = 0;
      setInterval(() => {
        if (k === 0) for (const c of list) c.classList.remove('on');
        list[k].classList.add('on');
        k = (k + 1) % n;
        count.textContent = (++total).toLocaleString('en-US') + ' blocks';
      }, ms);
    });

    // Lattice background: drifting points joined by faint edges (a nod to lattice/PQ math)
    const cv = document.getElementById('lattice');
    const cx = cv.getContext('2d');
    let W, H, pts = [], dpr = Math.min(devicePixelRatio || 1, 2);
    function resize() {
      W = cv.width = innerWidth * dpr; H = cv.height = innerHeight * dpr;
      cv.style.width = innerWidth + 'px'; cv.style.height = innerHeight + 'px';
      const n = Math.round(Math.min(90, innerWidth * innerHeight / 16000));
      pts = Array.from({ length: n }, () => ({
        x: Math.random() * W, y: Math.random() * H,
        vx: (Math.random() - 0.5) * 0.18 * dpr, vy: (Math.random() - 0.5) * 0.18 * dpr,
        g: Math.random() < 0.18
      }));
    }
    function frame() {
      cx.clearRect(0, 0, W, H);
      const max = 150 * dpr;
      for (let i = 0; i < pts.length; i++) {
        const a = pts[i];
        if (!reduce) { a.x += a.vx; a.y += a.vy; }
        if (a.x < 0 || a.x > W) a.vx *= -1;
        if (a.y < 0 || a.y > H) a.vy *= -1;
        for (let j = i + 1; j < pts.length; j++) {
          const b = pts[j], dx = a.x - b.x, dy = a.y - b.y, d = Math.hypot(dx, dy);
          if (d < max) {
            cx.strokeStyle = `rgba(139,92,246,${0.16 * (1 - d / max)})`;
            cx.lineWidth = dpr * 0.8;
            cx.beginPath(); cx.moveTo(a.x, a.y); cx.lineTo(b.x, b.y); cx.stroke();
          }
        }
        cx.fillStyle = a.g ? 'rgba(236,208,138,0.7)' : 'rgba(180,156,255,0.55)';
        cx.beginPath(); cx.arc(a.x, a.y, (a.g ? 1.6 : 1.1) * dpr, 0, Math.PI * 2); cx.fill();
      }
      if (!reduce) requestAnimationFrame(frame);
    }
    addEventListener('resize', () => { resize(); if (reduce) frame(); });
    resize(); frame();
