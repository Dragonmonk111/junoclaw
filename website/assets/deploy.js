    const checkboxes = document.querySelectorAll('.step input[type="checkbox"]');
    const progress = document.getElementById('progress');

    function updateProgress() {
      const checked = Array.from(checkboxes).filter(c => c.checked).length;
      const pct = (checked / checkboxes.length) * 100;
      progress.style.width = pct + '%';
    }

    checkboxes.forEach(cb => {
      const step = cb.closest('.step').dataset.step;
      cb.checked = localStorage.getItem('buzz-step-' + step) === 'true';
      cb.addEventListener('change', () => {
        localStorage.setItem('buzz-step-' + step, cb.checked);
        updateProgress();
      });
    });

    updateProgress();
