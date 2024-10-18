const { invoke } = window.__TAURI__.core;

window.startSimulation = async function () {
  try {
      const response = await invoke('start_simulation');
      console.log(response); // Log the response from the Rust command
      alert(response); // Show an alert with the response
  } catch (error) {
      console.error('Error starting simulation:', error);
      alert('Error starting simulation: ' + error);
  }
};

window.addEventListener("DOMContentLoaded", function() {
  function showContent(tabId) {
    console.log(tabId);

    const contents = document.querySelectorAll('.tab-content');
    contents.forEach(content => {
        content.classList.remove('active');
    });

    const selectedContent = document.getElementById(tabId);
    selectedContent.classList.add('active');
  }
  window.showContent = showContent;
});


let connectedControllers = [];

window.addEventListener("gamepadconnected", (event) => {
  handleConnectDisconnect(event, true);
});

window.addEventListener("gamepaddisconnected", (event) => {
  handleConnectDisconnect(event, false);
});

window.addEventListener("DOMContentLoaded", function() {
  function showContent(tabId) {
    console.log(tabId);

    const contents = document.querySelectorAll('.tab-content');
    contents.forEach(content => {
        content.classList.remove('active');
    });

    const selectedContent = document.getElementById(tabId);
    selectedContent.classList.add('active');
  }
  window.showContent = showContent;
});

function handleConnectDisconnect(event, connected) {
  const gamepad = event.gamepad;
  console.log(gamepad);

  if (connected) {
    connectedControllers[gamepad.index] = gamepad;
    updateUIForConnectedController(gamepad.index, true);
    createButtonLayout(gamepad.index, gamepad.buttons);
    createAxesLayout(gamepad.index, gamepad.axes);
  } else {
    connectedControllers[gamepad.index] = null;
    updateUIForConnectedController(gamepad.index, false);
  }
}

function updateUIForConnectedController(index, connected) {
  const controllerAreaNotConnected = document.getElementById(`controller-${index}-not-connected-area`);
  const controllerAreaConnected = document.getElementById(`controller-${index}-connected-area`);

  if (connected) {
    controllerAreaNotConnected.style.display = "none";
    controllerAreaConnected.style.display = "block";
  } else {
    controllerAreaNotConnected.style.display = "block";
    controllerAreaConnected.style.display = "none";
  }
}

function createAxesLayout(controllerIndex, axes) {
  const buttonsArea = document.getElementById(`buttons-controller-${controllerIndex}`);
  for (let i = 0; i < axes.length; i++) {
    buttonsArea.innerHTML += `<div id=axis-${controllerIndex}-${i} class='axis'>
                                 <div class='axis-name'>AXIS ${i}</div>
                                 <div class='axis-value'>${axes[i].toFixed(4)}</div>
                              </div> `;
  }
}

function createButtonLayout(controllerIndex, buttons) {
  const buttonArea = document.getElementById(`buttons-controller-${controllerIndex}`);
  buttonArea.innerHTML = "";
  for (let i = 0; i < buttons.length; i++) {
    buttonArea.innerHTML += createButtonHtml(controllerIndex, i, 0);
  }
}

function createButtonHtml(controllerIndex, index, value) {
  return `<div class="button" id="button-${controllerIndex}-${index}">
            <svg width="10px" height="50px">
                <rect width="10px" height="50px" fill="grey"></rect>
                <rect
                    class="button-meter"
                    width="10px"
                    x="0"
                    y="50"
                    data-original-y-position="50"
                    height="50px"
                    fill="rgb(60, 61, 60)"
                ></rect>
            </svg>
            <div class='button-text-area'>
                <div class="button-name">B${index}</div>
                <div class="button-value">${value.toFixed(2)}</div>
            </div>
        </div>`;
}

function updateButtonOnGrid(controllerIndex, index, value) {
  const buttonArea = document.getElementById(`button-${controllerIndex}-${index}`);
  const buttonValue = buttonArea.querySelector(".button-value");
  buttonValue.innerHTML = value.toFixed(2);

  const buttonMeter = buttonArea.querySelector(".button-meter");
  const meterHeight = Number(buttonMeter.dataset.originalYPosition);
  const meterPosition = meterHeight - (meterHeight / 100) * (value * 100);
  buttonMeter.setAttribute("y", meterPosition);
}

function updateControllerButton(controllerIndex, index, value) {
  const button = document.getElementById(`controller-${controllerIndex}-b${index}`);
  const selectedButtonClass = "selected-button";
  if (button) {
    if (value > 0) {
      button.classList.add(selectedButtonClass);
      button.style.filter = `contrast(${value * 200}%)`;
    } else {
      button.classList.remove(selectedButtonClass);
      button.style.filter = `contrast(100%)`;
    }
  }
}

function handleButtons(controllerIndex, buttons) {
  for (let i = 0; i < buttons.length; i++) {
    const buttonValue = buttons[i].value;
    updateButtonOnGrid(controllerIndex, i, buttonValue);
    updateControllerButton(controllerIndex, i, buttonValue);
  }
}

function handleSticks(controllerIndex, axes) {
  updateAxesGrid(controllerIndex, axes);
  updateStick(controllerIndex, "controller-b10", axes[0], axes[1]);
  updateStick(controllerIndex, "controller-b11", axes[2], axes[3]);
}

function updateAxesGrid(controllerIndex, axes) {
  for (let i = 0; i < axes.length; i++) {
    const axis = document.querySelector(`#axis-${controllerIndex}-${i} .axis-value`);
    const value = axes[i];
    axis.innerHTML = value.toFixed(4);
  }
}

function updateStick(controllerIndex, elementId, leftRightAxis, upDownAxis) {
  const multiplier = 25;
  const stickLeftRight = leftRightAxis * multiplier;
  const stickUpDown = upDownAxis * multiplier;

  const stick = document.getElementById(`${elementId}-${controllerIndex}`);
  const x = Number(stick.dataset.originalXPosition);
  const y = Number(stick.dataset.originalYPosition);

  stick.setAttribute("cx", x + stickLeftRight);
  stick.setAttribute("cy", y + stickUpDown);
}

function handleRumble(gamepad) {
  const rumbleOnButtonPress = document.getElementById("rumble-on-button-press");

  if (rumbleOnButtonPress.checked) {
    if (gamepad.buttons.some((button) => button.value > 0)) {
      gamepad.vibrationActuator.playEffect("dual-rumble", {
        startDelay: 0,
        duration: 25,
        weakMagnitude: 1.0,
        strongMagnitude: 1.0,
      });
    }
  }
}

function gameLoop() {
  connectedControllers.forEach((gamepad, index) => {
    const gamepadobj = navigator.getGamepads()[index];
    if (gamepadobj !== null) {
      handleButtons(index, gamepadobj.buttons);
      handleSticks(index, gamepadobj.axes);
      handleRumble(gamepadobj);
    }
  });
  requestAnimationFrame(gameLoop);
}

gameLoop();
