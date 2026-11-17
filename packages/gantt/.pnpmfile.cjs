// `ngc` (@angular/compiler-cli 22) needs TypeScript 6 and takes the `typescript` the package has, which is 7: give it its own copy (gantt#1875).
module.exports = {
  hooks: {
    readPackage(pkg) {
      if (pkg.name === "@angular/compiler-cli") {
        delete pkg.peerDependencies.typescript;
        pkg.dependencies = { ...pkg.dependencies, typescript: "npm:typescript@6.0.3" };
      }
      return pkg;
    },
  },
};
